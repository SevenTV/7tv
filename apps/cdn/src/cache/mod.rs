use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use axum::body::Body;
use axum::response::IntoResponse;
use bytes::{Bytes, BytesMut};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use shared::cdn::key::CacheKey;
use tokio::sync::OnceCell;

use crate::config;
use crate::global::Global;

const ONE_WEEK: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 24 * 7);

pub struct Cache {
	inner: moka::future::Cache<CacheKey, CachedResponse>,
	inflight: Arc<scc::HashMap<CacheKey, Arc<Inflight>>>,
	s3_client: aws_sdk_s3::client::Client,
	request_limiter: Arc<tokio::sync::Semaphore>,
	capacity: size::Size,
}

#[scuffle_metrics::metrics]
mod cache {
	use scuffle_metrics::{CounterU64, HistogramF64, MetricEnum, UpDownCounterI64};

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, MetricEnum)]
	pub enum State {
		Hit,
		ReboundHit,
		Coalesced,
		Miss,
	}

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, MetricEnum)]
	pub enum ResponseStatus {
		Success,
		NotFound,
		Timeout,
		InternalServerError,
	}

	pub fn action(state: State) -> CounterU64;

	pub fn upstream_response(status: ResponseStatus) -> CounterU64;

	pub fn inflight() -> UpDownCounterI64;

	pub fn duration() -> HistogramF64;

	pub struct InflightDropGuard(std::time::Instant);

	impl Drop for InflightDropGuard {
		fn drop(&mut self) {
			inflight().decr();
			duration().observe(self.0.elapsed().as_secs_f64());
		}
	}

	impl InflightDropGuard {
		pub fn new() -> Self {
			inflight().incr();
			Self(std::time::Instant::now())
		}
	}
}

impl Cache {
	pub fn new(config: &config::Cdn) -> Self {
		let s3_client = {
			let mut s3_config = if let Some(endpoint) = &config.bucket.endpoint {
				aws_sdk_s3::config::Builder::new().endpoint_url(endpoint)
			} else {
				aws_sdk_s3::config::Builder::new()
			}
			.region(aws_sdk_s3::config::Region::new(config.bucket.region.clone()))
			.force_path_style(true);

			if let Some(credentials) = config.bucket.credentials.to_credentials() {
				s3_config = s3_config.credentials_provider(credentials);
			}

			let config = s3_config.build();

			aws_sdk_s3::Client::from_conf(config)
		};

		let request_limiter = Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_requests as usize));

		let mut capacity = config.cache_capacity;
		if capacity.bytes() <= 0 {
			tracing::warn!(
				"cache capacity is set to 0, this will cause the cache to never expire, this will undoubtedly result in an OOM"
			);
			capacity = size::Size::from_bytes(u64::MAX);
		}

		Self {
			inner: moka::future::Cache::builder()
				.expire_after(CacheExpiry)
				.weigher(|k, v: &CachedResponse| {
					u32::try_from(v.data.len() + std::mem::size_of_val(v) + std::mem::size_of_val(k)).unwrap_or(u32::MAX)
				})
				.max_capacity(capacity.bytes() as u64)
				.build(),
			inflight: Arc::new(scc::HashMap::new()),
			s3_client,
			request_limiter,
			capacity,
		}
	}

	pub fn capacity(&self) -> u64 {
		self.capacity.bytes() as u64
	}

	pub fn entries(&self) -> u64 {
		self.inner.entry_count()
	}

	pub fn size(&self) -> u64 {
		self.inner.weighted_size()
	}

	pub fn inflight(&self) -> u64 {
		self.inflight.len() as u64
	}

	#[tracing::instrument(skip_all, name = "cache::purge", fields(key = %key))]
	pub async fn purge(&self, key: CacheKey) {
		tracing::info!("purging key");
		self.inner.invalidate(&key).await;
	}

	pub async fn handle_request(&self, global: &Arc<Global>, key: CacheKey) -> CachedResponse {
		if let Some(hit) = self.inner.get(&key).await {
			cache::action(cache::State::Hit).incr();

			// return cached response
			return hit;
		}

		let mut insert = false;

		let entry = Arc::clone(&global.cache.inflight.entry_async(key.clone()).await.or_insert_with(|| {
			insert = true;

			Arc::new(Inflight {
				token: tokio_util::sync::CancellationToken::new(),
				response: OnceCell::new(),
			})
		}));

		if !insert {
			tracing::debug!(key = %key, "pending");
			cache::action(cache::State::Coalesced).incr();
			// pending
			entry.token.cancelled().await;
			return entry.response.get().cloned().unwrap_or_else(CachedResponse::general_error);
		}

		struct PanicDropGuard(Option<(CacheKey, Arc<Inflight>, Arc<Global>)>);

		impl PanicDropGuard {
			fn new(key: CacheKey, entry: Arc<Inflight>, global: Arc<Global>) -> Self {
				Self(Some((key, entry, global)))
			}

			async fn disarm(mut self) {
				let Some((key, entry, global)) = self.0.take() else {
					return;
				};

				entry.token.cancel();
				global.cache.inflight.remove_async(&key).await;
			}

			fn entry(&self) -> &Arc<Inflight> {
				&self.0.as_ref().unwrap().1
			}

			fn global(&self) -> &Arc<Global> {
				&self.0.as_ref().unwrap().2
			}

			fn key(&self) -> &CacheKey {
				&self.0.as_ref().unwrap().0
			}
		}

		impl Drop for PanicDropGuard {
			fn drop(&mut self) {
				let Some((key, entry, global)) = self.0.take() else {
					return;
				};

				entry.token.cancel();
				global.cache.inflight.remove(&key);
			}
		}

		let guard = PanicDropGuard::new(key, entry, Arc::clone(global));

		if let Some(cached) = self.inner.get(guard.key()).await {
			tracing::debug!(key = %guard.key(), "rebounded hit");
			cache::action(cache::State::ReboundHit).incr();
			guard.entry().response.set(cached.clone()).expect("unreachable");
			guard.disarm().await;
			return cached.clone();
		}

		cache::action(cache::State::Miss).incr();

		let cached = tokio::spawn(async move {
			// request file
			let cached = guard.global().cache.request_key(guard.global(), guard.key()).await;

			guard.entry().response.set(cached.clone()).expect("unreachable");

			if !cached.max_age.is_zero() {
				guard.global().cache.inner.insert(guard.key().clone(), cached.clone()).await;
				tracing::debug!(key = %guard.key(), "cached");
			}

			guard.disarm().await;

			cached
		});

		cached.await.unwrap_or_else(|e| {
			tracing::error!(error = %e, "task failed");
			CachedResponse::general_error()
		})
	}

	async fn do_req(&self, global: &Arc<Global>, key: &CacheKey) -> Result<CachedResponse, S3ErrorWrapper> {
		let _inflight = cache::InflightDropGuard::new();
		let _permit = self.request_limiter.acquire().await.expect("semaphore closed");

		tracing::debug!(key = %key, "requesting origin");

		tokio::time::timeout(
			std::time::Duration::from_secs(global.config.cdn.origin_request_timeout),
			async {
				Ok(CachedResponse::from_s3_response(
					self.s3_client
						.get_object()
						.bucket(&global.config.cdn.bucket.name)
						.key(key.to_string())
						.send()
						.await?,
				)
				.await?)
			},
		)
		.await?
	}

	async fn request_key(&self, global: &Arc<Global>, key: &CacheKey) -> CachedResponse {
		match self.do_req(global, key).await {
			Ok(response) => {
				cache::upstream_response(cache::ResponseStatus::Success).incr();
				response
			}
			Err(S3ErrorWrapper::Sdk(aws_sdk_s3::error::SdkError::ServiceError(e))) if e.err().is_no_such_key() => {
				cache::upstream_response(cache::ResponseStatus::NotFound).incr();
				CachedResponse::not_found()
			}
			Err(S3ErrorWrapper::Timeout(_)) => {
				tracing::error!(key = %key, "timeout while requesting cdn file");
				cache::upstream_response(cache::ResponseStatus::Timeout).incr();
				CachedResponse::timeout()
			}
			Err(e) => {
				tracing::error!(key = %key, error = %e, "failed to request cdn file");
				cache::upstream_response(cache::ResponseStatus::InternalServerError).incr();
				CachedResponse::general_error()
			}
		}
	}
}

#[derive(Debug, thiserror::Error)]
enum S3ErrorWrapper {
	#[error("sdk error: {0}")]
	Sdk(#[from] aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>),
	#[error("timeout")]
	Timeout(#[from] tokio::time::error::Elapsed),
	#[error("bytes error: {0}")]
	Bytes(#[from] aws_sdk_s3::primitives::ByteStreamError),
}

/// Safe to clone
#[derive(Debug, Clone)]
pub struct Inflight {
	/// This token is pending as long as the request to the origin is pending.
	/// "Cancellation" is an unfortunate name for this because it is not used to
	/// cancel anything but rather notify everyone waiting that the cache is
	/// ready to be queried.
	token: tokio_util::sync::CancellationToken,
	/// The response once it is ready
	response: OnceCell<CachedResponse>,
}

#[derive(Debug, Clone)]
pub struct CachedResponse {
	pub data: CachedData,
	pub date: chrono::DateTime<chrono::Utc>,
	pub max_age: std::time::Duration,
	pub hits: Arc<AtomicUsize>,
}

impl CachedResponse {
	pub fn not_found() -> Self {
		Self {
			data: CachedData::NotFound,
			date: chrono::Utc::now(),
			max_age: std::time::Duration::from_secs(10),
			hits: Arc::new(AtomicUsize::new(0)),
		}
	}

	pub fn timeout() -> Self {
		Self {
			data: CachedData::InternalServerError,
			date: chrono::Utc::now(),
			max_age: std::time::Duration::ZERO,
			hits: Arc::new(AtomicUsize::new(0)),
		}
	}

	pub fn general_error() -> Self {
		Self {
			data: CachedData::InternalServerError,
			date: chrono::Utc::now(),
			max_age: std::time::Duration::ZERO,
			hits: Arc::new(AtomicUsize::new(0)),
		}
	}

	pub fn redirect(uri: String) -> Self {
		Self {
			data: CachedData::Redirect(uri),
			date: chrono::Utc::now(),
			max_age: std::time::Duration::ZERO,
			hits: Arc::new(AtomicUsize::new(0)),
		}
	}
}

#[derive(Debug, Clone)]
pub enum CachedData {
	Bytes { content_type: Option<String>, data: Bytes },
	Redirect(String),
	NotFound,
	InternalServerError,
}

impl CachedData {
	pub fn len(&self) -> usize {
		match self {
			Self::Bytes { data, .. } => data.len(),
			Self::Redirect(_) => 0,
			Self::NotFound => 0,
			Self::InternalServerError => 0,
		}
	}
}

impl IntoResponse for CachedData {
	fn into_response(self) -> axum::response::Response {
		match self {
			Self::Bytes { data, content_type } => {
				let mut headers = HeaderMap::new();

				if let Some(content_type) = content_type.as_deref().and_then(|c| c.try_into().ok()) {
					headers.insert(header::CONTENT_TYPE, content_type);
				}

				headers.insert(header::CONTENT_LENGTH, data.len().to_string().try_into().unwrap());

				(headers, Body::from(data)).into_response()
			}
			Self::Redirect(uri) => {
				let mut headers = HeaderMap::new();
				headers.insert(header::LOCATION, uri.try_into().unwrap());
				(headers, StatusCode::PERMANENT_REDIRECT).into_response()
			}
			Self::NotFound => StatusCode::NOT_FOUND.into_response(),
			Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
		}
	}
}

impl IntoResponse for CachedResponse {
	fn into_response(self) -> axum::response::Response {
		let mut data = self.data.into_response();

		if self.max_age.as_secs() == 0 {
			data.headers_mut()
				.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
		} else {
			let hits = self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

			let age = chrono::Utc::now() - self.date;
			data.headers_mut()
				.insert("x-7tv-cache-hits", hits.to_string().try_into().unwrap());
			data.headers_mut().insert(
				"x-7tv-cache",
				if hits == 0 {
					HeaderValue::from_static("miss")
				} else {
					HeaderValue::from_static("hit")
				},
			);

			data.headers_mut()
				.insert(header::AGE, age.num_seconds().to_string().try_into().unwrap());
			data.headers_mut().insert(
				header::CACHE_CONTROL,
				// We cache images for 1 week by default on the client however we want to purge intermediate caches
				// after 1 day to avoid stale content if we purge the CDN cache.
				format!(
					"public, max-age={}, s-maxage={}, immutable",
					self.max_age.as_secs(),
					self.max_age.as_secs().min(60 * 60 * 24)
				)
				.try_into()
				.unwrap(),
			);
		}

		data
	}
}

impl CachedResponse {
	pub async fn from_s3_response(
		mut value: aws_sdk_s3::operation::get_object::GetObjectOutput,
	) -> Result<Self, aws_sdk_s3::primitives::ByteStreamError> {
		let date = chrono::Utc::now();

		let max_age = value
			.cache_control
			.map(|c| c.to_ascii_lowercase())
			.as_deref()
			.and_then(|c| c.split(',').find_map(|v| v.strip_prefix("max-age=")))
			.and_then(|v| v.trim().parse::<u64>().ok())
			.map(std::time::Duration::from_secs)
			.or_else(|| {
				let expires = value
					.expires_string
					.and_then(|e| chrono::DateTime::parse_from_rfc2822(&e).ok());
				expires.and_then(|e| e.signed_duration_since(date).to_std().ok())
			})
			.unwrap_or(ONE_WEEK);

		let mut chunks = Vec::new();

		while let Some(chunk) = value.body.next().await.transpose()? {
			chunks.push(chunk);
		}

		let mut data = BytesMut::with_capacity(chunks.iter().map(|c| c.len()).sum());
		for chunk in chunks {
			data.extend_from_slice(&chunk);
		}

		Ok(Self {
			data: CachedData::Bytes {
				data: data.freeze(),
				content_type: value.content_type,
			},
			date,
			max_age,
			hits: Arc::new(AtomicUsize::new(0)),
		})
	}
}

struct CacheExpiry;

impl moka::Expiry<CacheKey, CachedResponse> for CacheExpiry {
	fn expire_after_create(
		&self,
		_key: &CacheKey,
		value: &CachedResponse,
		_created_at: std::time::Instant,
	) -> Option<std::time::Duration> {
		Some(value.max_age)
	}
}

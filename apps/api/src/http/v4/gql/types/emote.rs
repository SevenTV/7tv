use std::fmt::Display;
use std::sync::Arc;

use async_graphql::{ComplexObject, Context, Enum, SimpleObject};
use fred::prelude::KeysInterface;
use shared::database::emote::EmoteId;
use shared::database::user::UserId;
use shared::typesense::types::event::EventId;

use super::{EmoteEvent, Event, Image, SearchResult, User};
use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};
use crate::http::guards::RateLimitGuard;
use crate::search::{search, sorted_results, SearchOptions};

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct Emote {
	pub id: EmoteId,
	pub owner_id: UserId,
	pub default_name: String,
	pub tags: Vec<String>,
	pub images: Vec<Image>,
	pub flags: EmoteFlags,
	pub aspect_ratio: f64,
	pub attribution: Vec<EmoteAttribution>,
	pub scores: EmoteScores,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub search_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Enum)]
enum Ranking {
	TrendingDaily,
	TrendingWeekly,
	TrendingMonthly,
	TopDaily,
	TopWeekly,
	TopMonthly,
	TopAllTime,
}

impl Display for Ranking {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::TrendingDaily => write!(f, "trending_day"),
			Self::TrendingWeekly => write!(f, "trending_week"),
			Self::TrendingMonthly => write!(f, "trending_month"),
			Self::TopDaily => write!(f, "top_daily"),
			Self::TopWeekly => write!(f, "top_weekly"),
			Self::TopMonthly => write!(f, "top_monthly"),
			Self::TopAllTime => write!(f, "top_all_time"),
		}
	}
}

#[ComplexObject]
impl Emote {
	async fn owner<'ctx>(&self, ctx: &Context<'ctx>) -> Result<Option<User>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let user = global
			.user_loader
			.load(global, self.owner_id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?;

		Ok(user.map(Into::into))
	}

	async fn ranking<'ctx>(&self, ctx: &Context<'ctx>, ranking: Ranking) -> Result<Option<u32>, ApiError> {
		let global = ctx
			.data::<Arc<Global>>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let value: Option<String> = global.redis.get(format!("emote_stats:{ranking}")).await.map_err(|err| {
			tracing::error!(error = %err, "failed to get trending emote stats");
			ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to get trending emote stats")
		})?;

		let Some(value) = value else {
			return Ok(None);
		};

		let values: Vec<EmoteId> = serde_json::from_str(&value).map_err(|err| {
			tracing::error!(error = %err, "failed to parse trending emote stats");
			ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to parse trending emote stats")
		})?;

		Ok(values.into_iter().position(|e| e == self.id).map(|p| p as u32 + 1))
	}

	#[graphql(guard = "RateLimitGuard::search(1)")]
	async fn channels<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		#[graphql(validator(maximum = 10))] page: Option<u32>,
		#[graphql(validator(minimum = 1, maximum = 100))] per_page: Option<u32>,
	) -> Result<SearchResult<User>, ApiError> {
		let global = ctx
			.data::<Arc<Global>>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let per_page = per_page.unwrap_or(30);

		let options = SearchOptions::builder()
			.query("*".to_owned())
			.filter_by(format!("emotes: {}", self.id))
			.sort_by(vec!["role_hoist_rank:desc".to_owned()])
			.page(page)
			.per_page(per_page)
			.build();

		let result = search::<shared::typesense::types::user::User>(global, options)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to search");
				ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to search")
			})?;

		let users = global
			.user_loader
			.load_fast_many(global, result.hits.iter().copied())
			.await
			.map_err(|()| {
				tracing::error!("failed to load users");
				ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load users")
			})?;

		let result = SearchResult {
			items: sorted_results(result.hits, users).into_iter().map(Into::into).collect(),
			total_count: result.found,
			page_count: result.found.div_ceil(per_page as u64).min(10),
		};

		Ok(result)
	}

	#[graphql(guard = "RateLimitGuard::search(1)")]
	async fn events<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		#[graphql(validator(maximum = 10))] page: Option<u32>,
		#[graphql(validator(minimum = 1, maximum = 100))] per_page: Option<u32>,
	) -> Result<Vec<EmoteEvent>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let options = SearchOptions::builder()
			.query("*".to_owned())
			.filter_by(format!("target_id: {}", EventId::Emote(self.id)))
			.sort_by(vec!["created_at:desc".to_owned()])
			.page(page)
			.per_page(per_page.unwrap_or(20))
			.build();

		let result = search::<shared::typesense::types::event::Event>(global, options)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to search");
				ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to search")
			})?;

		let events = global
			.event_by_id_loader
			.load_many(result.hits.iter().copied())
			.await
			.map_err(|()| {
				tracing::error!("failed to load event");
				ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load event")
			})?;

		Ok(sorted_results(result.hits, events)
			.into_iter()
			.filter_map(|e| Event::try_from(e).ok())
			.collect())
	}
}

impl Emote {
	pub fn from_db(value: shared::database::emote::Emote, cdn_base_url: &url::Url) -> Self {
		Self {
			id: value.id,
			owner_id: value.owner_id,
			default_name: value.default_name,
			tags: value.tags,
			images: value
				.image_set
				.outputs
				.into_iter()
				.map(|o| Image::from_db(o, cdn_base_url))
				.collect(),
			flags: value.flags.into(),
			aspect_ratio: value.aspect_ratio,
			attribution: value.attribution.into_iter().map(Into::into).collect(),
			scores: value.scores.into(),
			updated_at: value.updated_at,
			search_updated_at: value.search_updated_at,
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
pub struct EmoteFlags {
	pub public_listed: bool,
	pub private: bool,
	pub nsfw: bool,
	pub default_zero_width: bool,
	pub approved_personal: bool,
	pub denied_personal: bool,
	pub animated: bool,
}

impl From<shared::database::emote::EmoteFlags> for EmoteFlags {
	fn from(value: shared::database::emote::EmoteFlags) -> Self {
		Self {
			public_listed: value.contains(shared::database::emote::EmoteFlags::PublicListed),
			private: value.contains(shared::database::emote::EmoteFlags::Private),
			nsfw: value.contains(shared::database::emote::EmoteFlags::Nsfw),
			default_zero_width: value.contains(shared::database::emote::EmoteFlags::DefaultZeroWidth),
			approved_personal: value.contains(shared::database::emote::EmoteFlags::ApprovedPersonal),
			denied_personal: value.contains(shared::database::emote::EmoteFlags::DeniedPersonal),
			animated: value.contains(shared::database::emote::EmoteFlags::Animated),
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
pub struct EmoteScores {
	pub trending_day: i32,
	pub trending_week: i32,
	pub trending_month: i32,
	pub top_daily: i32,
	pub top_weekly: i32,
	pub top_monthly: i32,
	pub top_all_time: i32,
}

impl From<shared::database::emote::EmoteScores> for EmoteScores {
	fn from(value: shared::database::emote::EmoteScores) -> Self {
		Self {
			trending_day: value.trending_day,
			trending_week: value.trending_week,
			trending_month: value.trending_month,
			top_daily: value.top_daily,
			top_weekly: value.top_weekly,
			top_monthly: value.top_monthly,
			top_all_time: value.top_all_time,
		}
	}
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct EmoteAttribution {
	pub user_id: UserId,
	pub added_at: chrono::DateTime<chrono::Utc>,
}

impl From<shared::database::emote::EmoteAttribution> for EmoteAttribution {
	fn from(value: shared::database::emote::EmoteAttribution) -> Self {
		Self {
			user_id: value.user_id,
			added_at: value.added_at,
		}
	}
}

#[ComplexObject]
impl EmoteAttribution {
	async fn user<'ctx>(&self, ctx: &Context<'ctx>) -> Result<Option<User>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let user = global
			.user_loader
			.load(global, self.user_id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?;

		Ok(user.map(Into::into))
	}
}

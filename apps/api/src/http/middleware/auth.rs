use std::sync::Arc;

use axum::extract::Request;
use axum::response::Response;
use futures::future::BoxFuture;
use futures::FutureExt;
use hyper::{header, StatusCode};
use mongodb::bson::doc;
use shared::database::{Collection, Permissions, User, UserId, UserSession};
use tokio::sync::OnceCell;

use super::cookies::Cookies;
use crate::global::Global;
use crate::http::error::{map_result, ApiError, EitherApiError};
use crate::jwt::{AuthJwtPayload, JwtState};

pub const AUTH_COOKIE: &str = "seventv-auth";

#[derive(Clone)]
pub struct AuthMiddleware(Arc<Global>);

impl AuthMiddleware {
	pub fn new(global: Arc<Global>) -> Self {
		Self(global)
	}
}

impl<S> tower::Layer<S> for AuthMiddleware {
	type Service = AuthMiddlewareService<S>;

	fn layer(&self, inner: S) -> Self::Service {
		AuthMiddlewareService {
			inner,
			global: self.0.clone(),
		}
	}
}

#[derive(Clone)]
pub struct AuthMiddlewareService<S> {
	global: Arc<Global>,
	inner: S,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
	pub kind: AuthSessionKind,
	/// lazy user data
	cached_data: Arc<OnceCell<(User, Permissions)>>,
}

#[derive(Debug, Clone)]
pub enum AuthSessionKind {
	/// The user session
	Session(UserSession),
	/// Old user sessions, only user id available
	Old(UserId),
}

impl AuthSession {
	pub fn user_id(&self) -> UserId {
		match &self.kind {
			AuthSessionKind::Session(session) => session.user_id,
			AuthSessionKind::Old(user_id) => *user_id,
		}
	}

	/// Lazy load user data
	pub async fn user(&self, global: &Arc<Global>) -> Result<&(User, Permissions), ApiError> {
		self.cached_data
			.get_or_try_init(|| async {
				let user = global
					.user_by_id_loader()
					.load(global, self.user_id())
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::UNAUTHORIZED)?;

				let global_config = global
					.global_config_loader()
					.load(())
					.await
					.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::INTERNAL_SERVER_ERROR)?;

				let roles = {
					let mut roles = global
						.role_by_id_loader()
						.load_many(user.entitled_cache.role_ids.iter().copied())
						.await
						.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?;

					global_config
						.role_ids
						.iter()
						.filter_map(|id| roles.remove(id))
						.collect::<Vec<_>>()
				};

				let permissions = user.compute_permissions(&roles);

				Ok((user, permissions))
			})
			.await
	}
}

impl<S> AuthMiddlewareService<S> {
	async fn serve<B>(mut self, mut req: Request<B>) -> Result<Response, EitherApiError<S::Error>>
	where
		S: tower::Service<Request<B>, Response = Response> + Clone + Send,
		S::Error: std::error::Error + Send,
		S::Future: Send,
		B: Send,
	{
		let cookies = req.extensions().get::<Cookies>().expect("cookies not found");
		let auth_cookie = cookies.get(AUTH_COOKIE);

		if let Some(token) = auth_cookie.as_ref().map(|c| c.value()).or_else(|| {
			req.headers()
				.get(header::AUTHORIZATION)
				.and_then(|v| v.to_str().ok())
				.map(|s| s.trim_start_matches("Bearer "))
		}) {
			let jwt = AuthJwtPayload::verify(&self.global, token).ok_or_else(|| {
				cookies.remove(AUTH_COOKIE);
				ApiError::UNAUTHORIZED
			})?;

			match jwt.session_id {
				Some(session_id) => {
					let session = UserSession::collection(self.global.db())
						.find_one_and_update(
							doc! {
								"_id": session_id,
								"expires_at": { "$gt": chrono::Utc::now() },
							},
							doc! {
								"$set": {
									"last_used_at": chrono::Utc::now(),
								},
							},
							Some(
								mongodb::options::FindOneAndUpdateOptions::builder()
									.return_document(mongodb::options::ReturnDocument::After)
									.upsert(false)
									.build(),
							),
						)
						.await
						.map_err(|err| {
							tracing::error!(error = %err, "failed to find user session");
							ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to find user session")
						})?
						.ok_or_else(|| {
							cookies.remove(AUTH_COOKIE);
							ApiError::new(StatusCode::UNAUTHORIZED, "session not found")
						})?;

					req.extensions_mut().insert(AuthSession {
						kind: AuthSessionKind::Session(session),
						cached_data: Arc::new(OnceCell::new()),
					});
				}
				// old session
				None => {
					req.extensions_mut().insert(AuthSession {
						kind: AuthSessionKind::Old(jwt.user_id),
						cached_data: Arc::new(OnceCell::new()),
					});
				}
			}
		}

		self.inner.call(req).await.map_err(EitherApiError::Other)
	}
}

impl<S, B> tower::Service<Request<B>> for AuthMiddlewareService<S>
where
	S: tower::Service<Request<B>, Response = Response> + Clone + Send + 'static,
	S::Error: std::error::Error + Send + 'static,
	S::Future: Send + 'static,
	B: Send + 'static,
{
	type Error = S::Error;
	type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
	type Response = S::Response;

	fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx).map_err(Into::into)
	}

	fn call(&mut self, req: Request<B>) -> Self::Future {
		Box::pin(self.clone().serve(req).map(map_result))
	}
}

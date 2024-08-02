use std::fmt::Display;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Extension, Router};
use hyper::StatusCode;
use mongodb::bson::doc;
use shared::database::event::EventUserSessionData;
use shared::database::queries::filter;
use shared::database::user::connection::Platform;
use shared::database::user::session::UserSession;
use shared::event::{EventPayload, EventPayloadData};

use self::login::{handle_callback as handle_login_callback, handle_login};
use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::extract::Query;
use crate::http::middleware::auth::{AuthSession, AuthSessionKind, AUTH_COOKIE};
use crate::http::middleware::cookies::Cookies;
use crate::transactions::{with_transaction, TransactionError};

mod login;

#[derive(utoipa::OpenApi)]
#[openapi(paths(login, logout, manual))]
pub struct Docs;

pub fn routes() -> Router<Arc<Global>> {
	Router::new()
		.route("/", get(login))
		.route("/logout", post(logout))
		.route("/manual", get(manual))
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoginRequestPlatform {
	Twitch,
	Discord,
	Youtube,
}

impl From<LoginRequestPlatform> for Platform {
	fn from(platform: LoginRequestPlatform) -> Self {
		match platform {
			LoginRequestPlatform::Twitch => Platform::Twitch,
			LoginRequestPlatform::Discord => Platform::Discord,
			LoginRequestPlatform::Youtube => Platform::Google,
		}
	}
}

impl Display for LoginRequestPlatform {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			LoginRequestPlatform::Twitch => write!(f, "twitch"),
			LoginRequestPlatform::Discord => write!(f, "discord"),
			LoginRequestPlatform::Youtube => write!(f, "youtube"),
		}
	}
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
	pub platform: LoginRequestPlatform,
	#[serde(default)]
	pub callback: bool,
	#[serde(default)]
	pub code: Option<String>,
	#[serde(default)]
	pub state: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v3/auth",
    tag = "auth",
    responses(
        (status = 303, description = "Auth Redirect"),
    ),
)]
#[tracing::instrument(
	skip_all,
	fields(
		query.platform = %query.platform,
		query.callback = %query.callback,
	)
)]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/auth/auth.route.go#L47
async fn login(
	State(global): State<Arc<Global>>,
	Extension(cookies): Extension<Cookies>,
	session: Option<AuthSession>,
	Query(query): Query<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
	let location = if query.callback {
		handle_login_callback(&global, query, &cookies).await?
	} else {
		let user_id = session.map(|s| s.user_id());
		handle_login(&global, user_id, query.platform.into(), &cookies)?
	};

	Ok(Redirect::to(&location))
}

#[utoipa::path(
    post,
    path = "/v3/auth/logout",
    tag = "auth",
    responses(
        (status = 204, description = "Logout"),
    ),
)]
#[tracing::instrument(skip(global, cookies))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/auth/logout.auth.route.go#L29
async fn logout(
	State(global): State<Arc<Global>>,
	Extension(cookies): Extension<Cookies>,
	auth_session: AuthSession,
) -> Result<impl IntoResponse, ApiError> {
	let res = with_transaction(&Arc::clone(&global), |mut tx| async move {
		// is a new session
		if let AuthSessionKind::Session(session) = &auth_session.kind {
			let user_session = tx
				.find_one_and_delete(
					filter::filter! {
						UserSession {
							#[query(rename = "_id")]
							id: session.id,
						}
					},
					None,
				)
				.await?;

			if let Some(user_session) = user_session {
				tx.register_event(EventPayload {
					actor_id: Some(auth_session.user_id()),
					data: EventPayloadData::UserSession {
						after: user_session,
						data: EventUserSessionData::Delete,
					},
					timestamp: chrono::Utc::now(),
				})?;
			}
		}

		cookies.remove(&global, AUTH_COOKIE);

		Ok(())
	})
	.await;

	match res {
		Ok(_) => Response::builder()
			.status(StatusCode::NO_CONTENT)
			.body(Body::empty())
			.map_err(|err| {
				tracing::error!(error = %err, "failed to create response");
				ApiError::INTERNAL_SERVER_ERROR
			}),
		Err(TransactionError::Custom(e)) => Err(e),
		Err(e) => {
			tracing::error!(error = %e, "transaction failed");
			Err(ApiError::INTERNAL_SERVER_ERROR)
		}
	}
}

#[utoipa::path(
    get,
    path = "/v3/auth/manual",
    tag = "auth",
    responses(
        (status = 200, description = "Manual Auth"),
    ),
)]
#[tracing::instrument]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/auth/manual.route.go#L41
async fn manual() -> Result<impl IntoResponse, ApiError> {
	// TODO: decide what to do here
	Ok(ApiError::NOT_IMPLEMENTED)
}

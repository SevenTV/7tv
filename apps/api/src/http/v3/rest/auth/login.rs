use std::sync::Arc;

use hyper::StatusCode;
use mongodb::bson::{doc, to_bson};
use shared::database::role::permissions::UserPermission;
use shared::database::user::connection::{Platform, UserConnection};
use shared::database::user::session::UserSession;
use shared::database::user::{User, UserId};
use shared::database::{Collection, Id};

use super::LoginRequest;
use crate::connections;
use crate::dataloader::user_loader::load_user_and_permissions;
use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::middleware::auth::AUTH_COOKIE;
use crate::http::middleware::cookies::{new_cookie, Cookies};
use crate::jwt::{AuthJwtPayload, CsrfJwtPayload, JwtState};

const CSRF_COOKIE: &str = "seventv-csrf";

const TWITCH_AUTH_URL: &str = "https://id.twitch.tv/oauth2/authorize?";
const TWITCH_AUTH_SCOPE: &str = "";

const DISCORD_AUTH_URL: &str = "https://discord.com/oauth2/authorize?";
const DISCORD_AUTH_SCOPE: &str = "identify";

const GOOGLE_AUTH_URL: &str =
	"https://accounts.google.com/o/oauth2/v2/auth?access_type=offline&include_granted_scopes=true&";
const GOOGLE_AUTH_SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";

pub async fn handle_callback(global: &Arc<Global>, query: LoginRequest, cookies: &Cookies) -> Result<String, ApiError> {
	let code = query
		.code
		.ok_or(ApiError::new_const(StatusCode::BAD_REQUEST, "missing code from query"))?;
	let state = query
		.state
		.ok_or(ApiError::new_const(StatusCode::BAD_REQUEST, "missing state from query"))?;

	// validate csrf
	let csrf_cookie = cookies
		.get(CSRF_COOKIE)
		.ok_or(ApiError::new_const(StatusCode::BAD_REQUEST, "missing csrf cookie"))?;

	let csrf_payload = CsrfJwtPayload::verify(global, csrf_cookie.value())
		.filter(|payload| payload.validate_random(&state).unwrap_or_default())
		.ok_or(ApiError::new_const(StatusCode::BAD_REQUEST, "invalid csrf"))?;

	// exchange code for access token
	let token = connections::exchange_code(
		global,
		query.platform.into(),
		&code,
		format!(
			"{}/v3/auth?callback=true&platform={}",
			global.config().api.api_origin,
			query.platform
		),
	)
	.await?;

	// query user data from platform
	let user_data = connections::get_user_data(global, query.platform.into(), &token.access_token).await?;

	let user_connection = global
		.user_connection_by_platform_id_loader()
		.load(user_data.id.clone())
		.await
		.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

	let mut session = global.mongo().start_session(None).await.map_err(|err| {
		tracing::error!(error = %err, "failed to start session");
		ApiError::INTERNAL_SERVER_ERROR
	})?;

	session.start_transaction(None).await.map_err(|err| {
		tracing::error!(error = %err, "failed to start transaction");
		ApiError::INTERNAL_SERVER_ERROR
	})?;
	
	let user_id = match (user_connection, csrf_payload.user_id) {
		(Some(user_connection), Some(user_id)) if user_connection.user_id != user_id => {
			return Err(ApiError::new_const(
				StatusCode::BAD_REQUEST,
				"connection already paired with another user",
			));
		},
		(None, None) => {
			// create new user

	let mut user = User::collection(global.db())
		.find_one_and_update_with_session(
			doc! {
				"connections.platform": query.platform,
				"connections.platform_id": &user_data.id,
			},
			doc! {
				"$set": {
					"connections.$.platform_username": &user_data.username,
					"connections.$.platform_display_name": &user_data.display_name,
					"connections.$.platform_avatar_url": &user_data.avatar,
					"connections.$.updated_at": chrono::Utc::now(),
				},
			},
			None,
			&mut session,
		)
		.await
		.map_err(|err| {
			tracing::error!(error = %err, "failed to find user");
			ApiError::INTERNAL_SERVER_ERROR
		})?;

	match (user, csrf_payload.user_id) {
		(Some(user), Some(user_id)) => {
			if user.id != user_id {
				return Err(ApiError::new_const(
					StatusCode::BAD_REQUEST,
					"connection already paired with another user",
				));
			}
		}
		(Some(user), None) => {
			let connection = user
				.connections
				.iter()
				.find(|c| c.platform == query.platform && c.platform_id == user_data.id)
				.ok_or_else(|| {
					tracing::error!("connection not found");
					ApiError::INTERNAL_SERVER_ERROR
				})?;

			if !connection.allow_login {
				return Err(ApiError::new_const(
					StatusCode::UNAUTHORIZED,
					"connection is not allowed to login",
				));
			}
		}
		(None, None) => {
			// New user creation
			user = Some(User {
				connections: vec![UserConnection {
					platform: query.platform,
					platform_id: user_data.id,
					platform_username: user_data.username,
					platform_display_name: user_data.display_name,
					platform_avatar_url: user_data.avatar,
					allow_login: true,
					updated_at: chrono::Utc::now(),
					..Default::default()
				}],
				..Default::default()
			});

			User::collection(global.db())
				.insert_one_with_session(user.as_ref().unwrap(), None, &mut session)
				.await
				.map_err(|err| {
					tracing::error!(error = %err, "failed to insert user");
					ApiError::INTERNAL_SERVER_ERROR
				})?;
		}
		_ => {}
	};

	let global_config = global
		.global_config_loader()
		.load(())
		.await
		.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
		.ok_or(ApiError::INTERNAL_SERVER_ERROR)?;

	let full_user = if let Some(user) = user {
		// This is correct for users that just got created aswell, as this will simply
		// load the default entitlements, as the user does not exist yet in the
		// database.
		global
			.user_loader()
			.load_user(user)
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
	} else if let Some(user_id) = csrf_payload.user_id {
		global
			.user_loader()
			.load(global, user_id)
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or_else(|| ApiError::new_const(StatusCode::BAD_REQUEST, "user not found"))?
	} else {
		unreachable!("user should be created or loaded");
	};

	if !full_user.computed.permissions.has(UserPermission::Login) {
		return Err(ApiError::new_const(StatusCode::FORBIDDEN, "not allowed to login"));
	}

	let logged_connection = full_user
		.connections
		.iter()
		.find(|c| c.platform == query.platform && c.platform_id == user_data.id);

	if let Some(logged_connection) = logged_connection {
		if logged_connection.platform_avatar_url != user_data.avatar
			|| logged_connection.platform_username != user_data.username
			|| logged_connection.platform_display_name != user_data.display_name
		{
			// Update user connection
			if User::collection(global.db())
				.update_one_with_session(
					doc! {
						"_id": full_user.user.id,
						"connections.platform": query.platform,
						"connections.platform_id": user_data.id,
					},
					doc! {
						"$set": {
							"connections.$.platform_username": &user_data.username,
							"connections.$.platform_display_name": &user_data.display_name,
							"connections.$.platform_avatar_url": &user_data.avatar,
							"connections.$.updated_at": chrono::Utc::now(),
							// This will trigger a search engine update
							"search_index.self_dirty": Id::new(),
						},
					},
					None,
					&mut session,
				)
				.await
				.map_err(|err| {
					tracing::error!(error = %err, "failed to update user");
					ApiError::INTERNAL_SERVER_ERROR
				})?
				.matched_count == 0
			{
				tracing::error!("failed to update user, no matched count");
				return Err(ApiError::INTERNAL_SERVER_ERROR);
			}
		}
	} else {
		if User::collection(global.db())
			.update_one_with_session(
				doc! {
					"_id": full_user.user.id,
				},
				doc! {
					"$push": {
						"connections": to_bson(&UserConnection {
							platform: query.platform,
							platform_id: user_data.id,
							platform_username: user_data.username,
							platform_display_name: user_data.display_name,
							platform_avatar_url: user_data.avatar,
							allow_login: true,
							updated_at: chrono::Utc::now(),
							linked_at: chrono::Utc::now(),
						}).unwrap(),
					},
					"$set": {
						"search_index.self_dirty": Id::new(),
					},
				},
				None,
				&mut session,
			)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to update user");
				ApiError::INTERNAL_SERVER_ERROR
			})?
			.modified_count
			== 0
		{
			tracing::error!("failed to insert user connection, no modified count");
			return Err(ApiError::INTERNAL_SERVER_ERROR);
		}
	}

	let user_session = if csrf_payload.user_id.is_none() {
		let user_session = UserSession {
			id: Default::default(),
			user_id: full_user.id,
			// TODO maybe allow for this to be configurable
			expires_at: chrono::Utc::now() + chrono::Duration::days(30),
			last_used_at: chrono::Utc::now(),
		};

		UserSession::collection(global.db())
			.insert_one_with_session(&user_session, None, &mut session)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to insert user session");
				ApiError::INTERNAL_SERVER_ERROR
			})?;

		Some(user_session)
	} else {
		None
	};

	session.commit_transaction().await.map_err(|err| {
		tracing::error!(error = %err, "failed to commit transaction");
		ApiError::INTERNAL_SERVER_ERROR
	})?;

	// create jwt access token
	let redirect_url = if let Some(user_session) = user_session {
		let jwt = AuthJwtPayload::from(user_session.clone());
		let token = jwt.serialize(global).ok_or_else(|| {
			tracing::error!("failed to serialize jwt");
			ApiError::INTERNAL_SERVER_ERROR
		})?;

		// create cookie
		let expiration =
			cookie::time::OffsetDateTime::from_unix_timestamp(user_session.expires_at.timestamp()).map_err(|err| {
				tracing::error!(error = %err, "failed to convert expiration to cookie time");
				ApiError::INTERNAL_SERVER_ERROR
			})?;

		cookies.add(new_cookie(global, (AUTH_COOKIE, token.clone())).expires(expiration));
		cookies.remove(&global, CSRF_COOKIE);

		format!(
			"{}/auth/callback?platform={}&token={}",
			global.config().api.website_origin,
			query.platform,
			token
		)
	} else {
		format!("{}/auth/callback?platform={}", global.config().api.website_origin, query.platform)
	};

	Ok(redirect_url)
}

pub fn handle_login(
	global: &Arc<Global>,
	user_id: Option<UserId>,
	platform: Platform,
	cookies: &Cookies,
) -> Result<String, ApiError> {
	// redirect to platform auth url
	let (url, scope, config) = match platform {
		Platform::Twitch => (TWITCH_AUTH_URL, TWITCH_AUTH_SCOPE, &global.config().api.connections.twitch),
		Platform::Discord => (DISCORD_AUTH_URL, DISCORD_AUTH_SCOPE, &global.config().api.connections.discord),
		Platform::Google => (GOOGLE_AUTH_URL, GOOGLE_AUTH_SCOPE, &global.config().api.connections.google),
		_ => return Err(ApiError::new_const(StatusCode::BAD_REQUEST, "unsupported platform")),
	};

	let csrf = CsrfJwtPayload::new(user_id);

	cookies.add(new_cookie(
		global,
		(
			CSRF_COOKIE,
			csrf.serialize(global).ok_or_else(|| {
				tracing::error!("failed to serialize csrf");
				ApiError::INTERNAL_SERVER_ERROR
			})?,
		),
	));

	let redirect_url = format!(
		"{}client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
		url,
		config.client_id,
		urlencoding::encode(&format!(
			"{}/v3/auth?callback=true&platform={}",
			global.config().api.api_origin,
			platform
		)),
		urlencoding::encode(scope),
		csrf.random()
	);

	Ok(redirect_url)
}

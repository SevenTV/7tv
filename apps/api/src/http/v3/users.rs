use std::sync::Arc;

use hyper::body::Incoming;
use hyper::StatusCode;
use scuffle_utils::http::ext::{OptionExt, ResultExt};
use scuffle_utils::http::router::builder::RouterBuilder;
use scuffle_utils::http::router::ext::RequestExt;
use scuffle_utils::http::router::Router;
use scuffle_utils::http::RouteError;
use shared::http::{json_response, Body};
use shared::id::parse_id;

use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::RequestGlobalExt;

#[derive(utoipa::OpenApi)]
#[openapi(
	paths(
		get_user_by_id,
		upload_user_profile_picture,
		get_user_presences_by_platform,
		get_user_by_platform_user_id,
		delete_user_by_id,
		update_user_connection_by_id,
	),
	components(schemas())
)]
pub struct Docs;

pub fn routes(_: &Arc<Global>) -> RouterBuilder<Incoming, Body, RouteError<ApiError>> {
	Router::builder()
		.get("/{id}", get_user_by_id)
		.put("/{id}/profile-picture", upload_user_profile_picture)
		.get("/{id}/presences", get_user_presences_by_platform)
		.get("/{platform}/{platform_id}", get_user_by_platform_user_id)
		.delete("/{id}", delete_user_by_id)
		.patch("/{id}/connections/{connection_id}", update_user_connection_by_id)
}

#[utoipa::path(
    get,
    path = "/v3/users/{id}",
    tag = "users",
    responses(
        (status = 200, description = "User", body = UserModel),
        // (status = 404, description = "User Not Found", body = ApiError)
    ),
    params(
        ("id" = String, Path, description = "The ID of the user"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.by-id.go#L44
pub async fn get_user_by_id(req: hyper::Request<Incoming>) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	let global: Arc<Global> = req.get_global()?;

	let id = req.param("id").map_err_route((StatusCode::BAD_REQUEST, "missing id"))?;
	let id = parse_id(id).map_err_route((StatusCode::BAD_REQUEST, "invalid id"))?;

	let user = global
		.user_by_id_loader()
		.load(&global, id)
		.await
		.map_ignore_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to fetch user"))?
		.map_err_route((StatusCode::NOT_FOUND, "user not found"))?
		.into_old_model(todo!(), todo!(), todo!(), todo!(), &global.config().api.cdn_base_url)
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to convert into old model"))?;

	json_response(user)
}

#[utoipa::path(
    put,
    path = "/v3/users/{id}/profile-picture",
    tag = "users",
    request_body(content = &[u8], description = "Image Binary Data", content_type = "image/*"),
    responses(
        (status = 200, description = "Success"),
    ),
    params(
        ("id" = String, Path, description = "The ID of the user"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.pictures.go#L61
pub async fn upload_user_profile_picture(
	req: hyper::Request<Incoming>,
) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	todo!()
}

#[utoipa::path(
    get,
    path = "/v3/users/{id}/presences",
    tag = "users",
    responses(
        (status = 200, description = "User Presences", body = Vec<UserModel>),
    ),
    params(
        ("id" = String, Path, description = "The ID of the user"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.presence.write.go#L41
pub async fn get_user_presences_by_platform(
	req: hyper::Request<Incoming>,
) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	todo!()
}

#[utoipa::path(
    get,
    path = "/v3/users/{platform}/{platform_id}",
    tag = "users",
    responses(
        (status = 200, description = "User", body = UserModel),
        // (status = 404, description = "User Not Found", body = ApiError)
    ),
    params(
        ("platform" = String, Path, description = "The platform"),
        ("platform_id" = String, Path, description = "The ID of the user on the platform"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.by-connection.go#L42
pub async fn get_user_by_platform_user_id(
	req: hyper::Request<Incoming>,
) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	todo!()
}

#[utoipa::path(
    delete,
    path = "/v3/users/{id}",
    tag = "users",
    responses(
        (status = 204, description = "User Deleted"),
        // (status = 404, description = "User Not Found", body = ApiError)
    ),
    params(
        ("id" = String, Path, description = "The ID of the user"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.delete.go#L33
pub async fn delete_user_by_id(req: hyper::Request<Incoming>) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	todo!()
}

#[utoipa::path(
    patch,
    path = "/v3/users/{id}/connections/{connection_id}",
    tag = "users",
    responses(
        (status = 200, description = "User Connection", body = UserConnectionModel),
        // (status = 404, description = "User Connection Not Found", body = ApiError)
    ),
    params(
        ("id" = String, Path, description = "The ID of the user"),
        ("connection_id" = String, Path, description = "The ID of the connection"),
    ),
)]
#[tracing::instrument(level = "info", skip(req), fields(path = %req.uri().path(), method = %req.method()))]
// https://github.com/SevenTV/API/blob/c47b8c8d4f5c941bb99ef4d1cfb18d0dafc65b97/internal/api/rest/v3/routes/users/users.update-connection.go#L34
pub async fn update_user_connection_by_id(
	req: hyper::Request<Incoming>,
) -> Result<hyper::Response<Body>, RouteError<ApiError>> {
	todo!()
}

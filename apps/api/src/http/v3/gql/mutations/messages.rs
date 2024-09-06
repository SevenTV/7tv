use std::sync::Arc;

use async_graphql::{Context, Object};
use shared::database::emote_moderation_request::{
	EmoteModerationRequest, EmoteModerationRequestId, EmoteModerationRequestStatus,
};
use shared::database::queries::{filter, update};
use shared::database::role::permissions::{EmoteModerationRequestPermission, PermissionsExt};
use shared::database::MongoCollection;
use shared::old_types::object_id::GqlObjectId;

use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::middleware::auth::AuthSession;
use crate::http::v3::gql::queries::message::InboxMessage;

// https://github.com/SevenTV/API/blob/main/internal/api/gql/v3/resolvers/mutation/mutation.messages.go

#[derive(Default)]
pub struct MessagesMutation;

#[Object(rename_fields = "camelCase", rename_args = "snake_case")]
impl MessagesMutation {
	async fn read_messages<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		message_ids: Vec<GqlObjectId>,
		read: bool,
		approved: bool,
	) -> Result<u32, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let authed_user = auth_session.user(global).await?;

		if !authed_user.has(EmoteModerationRequestPermission::Manage) {
			return Err(ApiError::FORBIDDEN);
		}

		let ids: Vec<EmoteModerationRequestId> = message_ids.into_iter().map(|id| id.id()).collect();

		// read is false when mods click the undo button
		let status = if read {
			if approved {
				EmoteModerationRequestStatus::Approved
			} else {
				EmoteModerationRequestStatus::Denied
			}
		} else {
			EmoteModerationRequestStatus::Pending
		};

		let res = EmoteModerationRequest::collection(&global.db)
			.update_many(
				filter::filter! {
					EmoteModerationRequest {
						#[query(rename = "_id", selector = "in")]
						id: ids,
					}
				},
				update::update! {
					#[query(set)]
					EmoteModerationRequest {
						#[query(serde)]
						status: status,
					}
				},
			)
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to update moderation requests");
				ApiError::INTERNAL_SERVER_ERROR
			})?;

		Ok(res.modified_count as u32)
	}

	async fn send_inbox_message(
		&self,
		_recipients: Vec<GqlObjectId>,
		_subject: String,
		_content: String,
		_important: Option<bool>,
		_anonymous: Option<bool>,
	) -> Result<Option<InboxMessage>, ApiError> {
		// will be left unimplemented
		Err(ApiError::NOT_IMPLEMENTED)
	}

	async fn dismiss_void_target_mod_requests(&self, _object: u32) -> Result<u32, ApiError> {
		// will be left unimplemented
		Err(ApiError::NOT_IMPLEMENTED)
	}
}

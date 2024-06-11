use std::sync::Arc;

use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};
use mongodb::bson::doc;
use shared::database::{self, Collection, EmotePermission, UserEditorState};
use shared::old_types::{EmoteFlagsModel, EmoteObjectId, UserObjectId};

use crate::global::Global;
use crate::http::middleware::auth::AuthSession;
use crate::http::v3::gql::guards::PermissionGuard;
use crate::http::{error::ApiError, v3::gql::queries::Emote};

#[derive(Default)]
pub struct EmotesMutation;

#[Object(rename_fields = "camelCase", rename_args = "snake_case")]
impl EmotesMutation {
	async fn emote<'ctx>(&self, ctx: &Context<'ctx>, id: EmoteObjectId) -> Result<EmoteOps, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let emote = global
			.emote_by_id_loader()
			.load(id.id())
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or(ApiError::NOT_FOUND)?;

		Ok(EmoteOps { id, _emote: emote })
	}
}

#[derive(SimpleObject)]
#[graphql(complex, rename_fields = "snake_case")]
pub struct EmoteOps {
	id: EmoteObjectId,
	#[graphql(skip)]
	_emote: database::Emote,
}

#[ComplexObject(rename_fields = "camelCase", rename_args = "snake_case")]
impl EmoteOps {
	async fn update<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		params: EmoteUpdate,
		_reason: Option<String>,
	) -> Result<Emote, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;
		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let (user, perms) = auth_session.user(global).await?;

		let editors = if let Some(owner_id) = self._emote.owner_id {
			global
				.user_editor_by_user_id_loader()
				.load(owner_id)
				.await
				.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
				.unwrap_or_default()
		} else {
			Vec::new()
		};

		if !(self._emote.owner_id == Some(user.id)
			|| perms.has(EmotePermission::Admin)
			|| editors.iter().any(|editor| {
				editor.state == UserEditorState::Accepted
					&& editor.user_id == auth_session.user_id()
					&& editor.permissions.has_emote(EmotePermission::Edit)
			})) {
			return Err(ApiError::FORBIDDEN);
		}

		if params.deleted.is_some_and(|d| d) {
			if !perms.has(EmotePermission::Delete) {
				return Err(ApiError::FORBIDDEN);
			}

			let emote = database::Emote::collection(global.db())
				.find_one_and_delete(doc! { "_id": self.id.id() }, None)
				.await
				.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
				.ok_or(ApiError::NOT_FOUND)?;

			Ok(Emote::from_db(global, emote))
		} else {
			if !perms.has(EmotePermission::Edit) {
				return Err(ApiError::FORBIDDEN);
			}

			let mut update = doc! {};

			if let Some(name) = params.name.or(params.version_name) {
				update.insert("default_name", name);
			}

			if let Some(tags) = params.tags {
				update.insert("tags", tags);
			}

			let mut flags = self._emote.flags;

			if let Some(input_flags) = params.flags {
				if input_flags.contains(EmoteFlagsModel::Private) {
					flags |= database::EmoteFlags::Private;
					flags &= !database::EmoteFlags::PublicListed;
				} else {
					flags &= !database::EmoteFlags::Private;
					flags |= database::EmoteFlags::PublicListed;
				}

				if input_flags.contains(EmoteFlagsModel::ZeroWidth) {
					flags |= database::EmoteFlags::DefaultZeroWidth;
				} else {
					flags &= !database::EmoteFlags::DefaultZeroWidth;
				}
			}

			// changing visibility and owner requires admin perms
			if perms.has(EmotePermission::Admin) {
				if let Some(listed) = params.listed {
					if listed {
						flags |= database::EmoteFlags::PublicListed;
						flags &= !database::EmoteFlags::Private;
					} else {
						flags &= !database::EmoteFlags::PublicListed;
						flags |= database::EmoteFlags::Private;
					}
				}

				if let Some(personal_use) = params.personal_use {
					if personal_use {
						flags |= database::EmoteFlags::ApprovedPersonal;
						flags &= !database::EmoteFlags::DeniedPersonal;
					} else {
						flags &= !database::EmoteFlags::ApprovedPersonal;
						flags |= database::EmoteFlags::DeniedPersonal;
					}
				}

				if let Some(owner_id) = params.owner_id {
					update.insert("owner_id", owner_id.id());
				}
			}

			update.insert("flags", flags.bits() as u32);

			let emote = database::Emote::collection(global.db())
				.find_one_and_update(doc! { "_id": self.id.id() }, doc! { "$set": update }, None)
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to update emote");
					ApiError::INTERNAL_SERVER_ERROR
				})?
				.ok_or(ApiError::NOT_FOUND)?;

			Ok(Emote::from_db(global, emote))
		}
	}

	#[graphql(guard = "PermissionGuard::one(EmotePermission::Admin)")]
	async fn merge<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		target_id: EmoteObjectId,
		_reason: Option<String>,
	) -> Result<Emote, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let emote = database::Emote::collection(global.db())
			.find_one_and_update(
				doc! { "_id": self.id.id() },
				doc! {
					"$set": {
						"merged_into": target_id.id(),
					},
					"$currentDate": {
						"merged_at": { "$type": "date" },
					},
				},
				None,
			)
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to update emote");
				ApiError::INTERNAL_SERVER_ERROR
			})?
			.ok_or(ApiError::NOT_FOUND)?;

        // TODO: schedule emote merge job

		Ok(Emote::from_db(global, emote))
	}

	#[graphql(guard = "PermissionGuard::one(EmotePermission::Admin)")]
	async fn rerun(&self) -> Result<Option<Emote>, ApiError> {
		Err(ApiError::NOT_IMPLEMENTED)
	}
}

#[derive(InputObject)]
#[graphql(rename_fields = "snake_case")]
pub struct EmoteUpdate {
	name: Option<String>,
	version_name: Option<String>,
	version_description: Option<String>,
	flags: Option<EmoteFlagsModel>,
	owner_id: Option<UserObjectId>,
	tags: Option<Vec<String>>,
	listed: Option<bool>,
	personal_use: Option<bool>,
	deleted: Option<bool>,
}

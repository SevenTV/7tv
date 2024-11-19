use std::sync::Arc;

use async_graphql::{ComplexObject, Context, SimpleObject};
use shared::database::emote_set::EmoteSetId;
use shared::database::role::permissions::{PermissionsExt, UserPermission};
use shared::database::role::RoleId;
use shared::database::user::editor::EditorEmoteSetPermission;
use shared::database::user::UserId;

use super::{Color, Emote, EmoteSet, Role, UserEditor};
use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};
use crate::http::middleware::session::Session;

pub mod connection;
pub mod inventory;
pub mod style;

pub use connection::*;
pub use inventory::*;
pub use style::*;

#[derive(Debug, Clone, SimpleObject)]
#[graphql(complex)]
pub struct User {
	pub id: UserId,
	pub connections: Vec<UserConnection>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub search_updated_at: Option<chrono::DateTime<chrono::Utc>>,

	// Computed fields
	pub highest_role_rank: i32,
	pub highest_role_color: Option<Color>,
	pub role_ids: Vec<RoleId>,

	#[graphql(skip)]
	full_user: shared::database::user::FullUser,
}

#[ComplexObject]
impl User {
	async fn main_connection(&self) -> Option<&UserConnection> {
		self.connections.first()
	}

	// TODO: Does it make sense to paginate this?
	async fn owned_emotes(&self, ctx: &Context<'_>) -> Result<Vec<Emote>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let mut emotes = global
			.emote_by_user_id_loader
			.load(self.id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load emotes"))?
			.unwrap_or_default();

		emotes.sort_by(|a, b| a.id.cmp(&b.id));

		Ok(emotes
			.into_iter()
			.map(|e| Emote::from_db(e, &global.config.api.cdn_origin))
			.collect())
	}

	async fn owned_emote_sets(&self, ctx: &Context<'_>) -> Result<Vec<EmoteSet>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let mut emote_sets = global
			.emote_set_by_user_id_loader
			.load(self.id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load emote sets"))?
			.unwrap_or_default();

		emote_sets.sort_by(|a, b| a.id.cmp(&b.id));

		Ok(emote_sets.into_iter().map(Into::into).collect())
	}

	async fn style(&self, ctx: &Context<'_>) -> Result<UserStyle, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		Ok(UserStyle::from_user(global, &self.full_user))
	}

	async fn roles(&self, ctx: &Context<'_>) -> Result<Vec<Role>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let mut loaded = global
			.role_by_id_loader
			.load_many(self.role_ids.iter().copied())
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load roles"))?;

		let mut roles = Vec::with_capacity(loaded.len());

		for id in &self.role_ids {
			if let Some(role) = loaded.remove(id) {
				roles.push(role);
			}
		}

		Ok(roles.into_iter().map(Into::into).collect())
	}

	async fn editors(&self, ctx: &Context<'_>) -> Result<Vec<UserEditor>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let editors = global
			.user_editor_by_user_id_loader
			.load(self.id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editors"))?
			.unwrap_or_default();

		Ok(editors.into_iter().map(Into::into).collect())
	}

	async fn editable_emote_set_ids(&self, ctx: &Context<'_>) -> Result<Vec<EmoteSetId>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let session = ctx
			.data::<Session>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing session data"))?;
		let authed_user = session.user()?;

		if authed_user.id != self.id && !authed_user.has(UserPermission::ManageAny) {
			return Err(ApiError::forbidden(
				ApiErrorCode::LackingPrivileges,
				"you are not allowed to see this user's emote sets",
			));
		}

		let owners = global
			.user_editor_by_editor_id_loader
			.load(self.id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editors"))?
			.unwrap_or_default()
			.into_iter()
			.filter(|editor| editor.permissions.has_emote_set(EditorEmoteSetPermission::Manage))
			.map(|editor| editor.id.user_id)
			.chain(std::iter::once(self.id));

		let mut emote_sets: Vec<EmoteSetId> = global
			.emote_set_by_user_id_loader
			.load_many(owners)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load emote sets"))?
			.into_values()
			.flatten()
			.map(|e| e.id)
			.collect();

		emote_sets.sort();

		Ok(emote_sets)
	}

	async fn inventory(&self) -> UserInventory {
		UserInventory::from_user(&self.full_user)
	}
}

impl From<shared::database::user::FullUser> for User {
	fn from(value: shared::database::user::FullUser) -> Self {
		Self {
			id: value.id,
			connections: value.connections.iter().cloned().map(Into::into).collect(),
			updated_at: value.updated_at,
			search_updated_at: value.search_updated_at,
			highest_role_rank: value.computed.highest_role_rank,
			highest_role_color: value.computed.highest_role_color.map(Color),
			role_ids: value.computed.roles.clone(),
			full_user: value,
		}
	}
}

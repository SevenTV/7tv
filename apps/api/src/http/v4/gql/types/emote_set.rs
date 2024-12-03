use std::sync::Arc;

use async_graphql::Context;
use shared::database::emote::EmoteId;
use shared::database::emote_set::EmoteSetId;
use shared::database::user::UserId;

use super::{Emote, SearchResult, User};
use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};

#[derive(Debug, Clone, async_graphql::SimpleObject)]
#[graphql(complex)]
pub struct EmoteSet {
	pub id: EmoteSetId,
	pub name: String,
	pub description: Option<String>,
	pub tags: Vec<String>,
	pub capacity: Option<i32>,
	pub owner_id: Option<UserId>,
	pub kind: EmoteSetKind,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub search_updated_at: Option<chrono::DateTime<chrono::Utc>>,

	#[graphql(skip)]
	pub emotes: Vec<EmoteSetEmote>,
}

#[async_graphql::ComplexObject]
impl EmoteSet {
	#[tracing::instrument(skip_all, name = "EmoteSet::emotes")]
	async fn emotes(
		&self,
		page: Option<u32>,
		#[graphql(validator(minimum = 1))] per_page: Option<u32>,
	) -> SearchResult<EmoteSetEmote> {
		if let Some(page) = page {
			let chunk_size = per_page.map(|p| p as usize).unwrap_or(20);

			let items = self
				.emotes
				.chunks(chunk_size)
				.nth(page.saturating_sub(1) as usize)
				.unwrap_or_default()
				.to_vec();

			SearchResult {
				items,
				total_count: self.emotes.len() as u64,
				page_count: (self.emotes.len() as u64 / chunk_size as u64) + 1,
			}
		} else {
			SearchResult {
				items: self.emotes.clone(),
				total_count: self.emotes.len() as u64,
				page_count: 1,
			}
		}
	}

	#[tracing::instrument(skip_all, name = "EmoteSet::owner")]
	async fn owner(&self, ctx: &Context<'_>) -> Result<Option<User>, ApiError> {
		let Some(user_id) = self.owner_id else {
			return Ok(None);
		};

		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let user = global
			.user_loader
			.load(global, user_id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?;

		Ok(user.map(Into::into))
	}
}

impl From<shared::database::emote_set::EmoteSet> for EmoteSet {
	fn from(value: shared::database::emote_set::EmoteSet) -> Self {
		Self {
			id: value.id,
			name: value.name,
			description: value.description,
			tags: value.tags,
			emotes: value.emotes.into_iter().map(Into::into).collect(),
			capacity: value.capacity,
			owner_id: value.owner_id,
			kind: value.kind.into(),
			updated_at: value.updated_at,
			search_updated_at: value.search_updated_at,
		}
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, async_graphql::Enum)]
pub enum EmoteSetKind {
	Normal,
	Personal,
	Global,
	Special,
}

impl From<shared::database::emote_set::EmoteSetKind> for EmoteSetKind {
	fn from(value: shared::database::emote_set::EmoteSetKind) -> Self {
		match value {
			shared::database::emote_set::EmoteSetKind::Normal => Self::Normal,
			shared::database::emote_set::EmoteSetKind::Personal => Self::Personal,
			shared::database::emote_set::EmoteSetKind::Global => Self::Global,
			shared::database::emote_set::EmoteSetKind::Special => Self::Special,
		}
	}
}

#[derive(Debug, Clone, async_graphql::SimpleObject)]
#[graphql(complex)]
pub struct EmoteSetEmote {
	pub id: EmoteId,
	pub alias: String,
	pub added_at: chrono::DateTime<chrono::Utc>,
	pub flags: EmoteSetEmoteFlags,
	pub added_by_id: Option<UserId>,
	pub origin_set_id: Option<EmoteSetId>,
}

#[async_graphql::ComplexObject]
impl EmoteSetEmote {
	#[tracing::instrument(skip_all, name = "EmoteSetEmote::emote")]
	async fn emote(&self, ctx: &Context<'_>) -> Result<Option<Emote>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;

		let emote = global
			.emote_by_id_loader
			.load(self.id)
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load emote"))?;

		Ok(emote.map(|e| Emote::from_db(e, &global.config.api.cdn_origin)))
	}
}

impl From<shared::database::emote_set::EmoteSetEmote> for EmoteSetEmote {
	fn from(value: shared::database::emote_set::EmoteSetEmote) -> Self {
		Self {
			id: value.id,
			alias: value.alias,
			added_at: value.added_at,
			flags: value.flags.into(),
			added_by_id: value.added_by_id,
			origin_set_id: value.origin_set_id,
		}
	}
}

#[derive(Debug, Clone, async_graphql::SimpleObject)]
pub struct EmoteSetEmoteFlags {
	pub zero_width: bool,
	pub override_conflicts: bool,
}

impl From<shared::database::emote_set::EmoteSetEmoteFlag> for EmoteSetEmoteFlags {
	fn from(value: shared::database::emote_set::EmoteSetEmoteFlag) -> Self {
		Self {
			zero_width: value.contains(shared::database::emote_set::EmoteSetEmoteFlag::ZeroWidth),
			override_conflicts: value.contains(shared::database::emote_set::EmoteSetEmoteFlag::OverrideConflicts),
		}
	}
}

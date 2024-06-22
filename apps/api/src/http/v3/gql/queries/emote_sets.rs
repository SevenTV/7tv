use std::sync::Arc;

use async_graphql::{ComplexObject, Context, Enum, Object, SimpleObject};
use hyper::StatusCode;
use mongodb::bson::doc;
use shared::database::{Collection, EmoteId, EmoteSetEmote, User, UserConnection, UserId};
use shared::old_types::{
	ActiveEmoteFlagModel, EmoteObjectId, EmoteSetFlagModel, EmoteSetObjectId, ObjectId, UserObjectId, VirtualId,
};

use super::emotes::{Emote, EmotePartial};
use super::users::UserPartial;
use crate::dataloader::user_loader::{load_user_and_permissions, load_users_and_permissions};
use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::v3::emote_set_loader::{get_virtual_set_emotes_for_user, virtual_user_set};

// https://github.com/SevenTV/API/blob/main/internal/api/gql/v3/schema/emoteset.gql

#[derive(Default)]
pub struct EmoteSetsQuery;

#[derive(Debug, Clone, Default, SimpleObject)]
#[graphql(complex, rename_fields = "snake_case")]
pub struct EmoteSet {
	id: EmoteSetObjectId,
	name: String,
	flags: EmoteSetFlagModel,
	tags: Vec<String>,
	// emotes
	// emote_count
	// capacity
	origins: Vec<EmoteSetOrigin>, // always empty
	owner_id: Option<UserObjectId>,
	// owner
	#[graphql(skip)]
	virtual_set_of: Option<(User, u16)>,
}

impl EmoteSet {
	pub fn from_db(value: shared::database::EmoteSet) -> Self {
		Self {
			flags: EmoteSetFlagModel::from_db(&value),
			id: value.id.into(),
			name: value.name,
			tags: value.tags,
			origins: Vec::new(),
			owner_id: value.owner_id.map(Into::into),
			virtual_set_of: None,
		}
	}

	pub async fn virtual_set_for_user(user: User, user_connections: Vec<UserConnection>, slots: u16) -> Self {
		let display_name = user_connections
			.iter()
			.find(|conn| conn.main_connection)
			.map(|c| c.platform_display_name.clone());

		let mut set = Self::from_db(virtual_user_set(user.id, display_name, slots));

		set.id = EmoteSetObjectId::VirtualId(VirtualId(user.id));
		set.virtual_set_of = Some((user, slots));

		set
	}
}

#[derive(Debug, Clone, Default, SimpleObject, serde::Deserialize, serde::Serialize)]
#[graphql(complex, rename_fields = "snake_case")]
pub struct ActiveEmote {
	pub id: EmoteObjectId,
	pub name: String,
	pub flags: ActiveEmoteFlagModel,
	// timestamp
	// data
	// actor
	pub origin_id: Option<ObjectId<()>>,

	#[graphql(skip)]
	pub emote_id: EmoteId,
	#[graphql(skip)]
	pub actor_id: Option<UserId>,
}

impl ActiveEmote {
	pub fn from_db(value: EmoteSetEmote) -> Self {
		Self {
			id: value.emote_id.into(),
			name: value.name,
			flags: value.flags.into(),
			emote_id: value.emote_id,
			actor_id: value.added_by_id,
			origin_id: None,
		}
	}
}

#[ComplexObject(rename_fields = "snake_case", rename_args = "snake_case")]
impl ActiveEmote {
	async fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
		self.id.id().timestamp()
	}

	async fn data<'ctx>(&self, ctx: &Context<'ctx>) -> Result<EmotePartial, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let emote = global
			.emote_by_id_loader()
			.load(self.emote_id)
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		Ok(emote
			.map(|e| Emote::from_db(global, e))
			.unwrap_or_else(Emote::deleted_emote)
			.into())
	}

	async fn actor<'ctx>(&self, ctx: &Context<'ctx>) -> Result<Option<UserPartial>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		if let Some(actor_id) = self.actor_id {
			Ok(UserPartial::load_from_db(global, actor_id).await?)
		} else {
			Ok(None)
		}
	}
}

#[ComplexObject(rename_fields = "snake_case", rename_args = "snake_case")]
impl EmoteSet {
	async fn emotes<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		limit: Option<u32>,
		_origins: Option<bool>,
	) -> Result<Vec<ActiveEmote>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let emote_set_emotes = match &self.virtual_set_of {
			Some((user, slots)) => get_virtual_set_emotes_for_user(global, user, *slots).await?,
			None => global
				.emote_set_emote_by_id_loader()
				.load(self.id.id())
				.await
				.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
				.unwrap_or_default(),
		};

		let emotes = match limit {
			Some(limit) => emote_set_emotes
				.into_iter()
				.take(limit as usize)
				.map(ActiveEmote::from_db)
				.collect(),
			None => emote_set_emotes.into_iter().map(ActiveEmote::from_db).collect(),
		};

		Ok(emotes)
	}

	async fn emote_count<'ctx>(&self, ctx: &Context<'ctx>) -> Result<u32, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		match &self.virtual_set_of {
			Some((user, slots)) => Ok(get_virtual_set_emotes_for_user(global, user, *slots).await?.len() as u32),
			None => {
				let count = EmoteSetEmote::collection(global.db())
					.count_documents(
						doc! {
							"emote_set_id": self.id.id(),
						},
						None,
					)
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;
				Ok(count as u32)
			}
		}
	}

	async fn capacity<'ctx>(&self, ctx: &Context<'ctx>) -> Result<u16, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let Some(owner_id) = self.owner_id else {
			return Ok(600);
		};

		let perms = load_user_and_permissions(global, owner_id.id()).await?.map(|(_, p)| p);

		Ok(perms.and_then(|p| p.emote_set_slots_limit).unwrap_or(600))
	}

	async fn owner<'ctx>(&self, ctx: &Context<'ctx>) -> Result<Option<UserPartial>, ApiError> {
		let Some(id) = self.owner_id else {
			return Ok(None);
		};

		let global = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;
		Ok(UserPartial::load_from_db(global, id.id()).await?)
	}
}

#[derive(Debug, Clone, Default, async_graphql::SimpleObject)]
#[graphql(rename_fields = "snake_case")]
pub struct EmoteSetOrigin {
	id: ObjectId<()>,
	weight: i32,
	slices: Vec<i32>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Enum)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum EmoteSetName {
	Global,
}

#[Object(rename_fields = "camelCase", rename_args = "snake_case")]
impl EmoteSetsQuery {
	async fn emote_set<'ctx>(&self, ctx: &Context<'ctx>, id: EmoteSetObjectId) -> Result<EmoteSet, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		match id {
			EmoteSetObjectId::Id(id) => {
				let emote_set = global
					.emote_set_by_id_loader()
					.load(id)
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::NOT_FOUND)?;

				Ok(EmoteSet::from_db(emote_set))
			}
			EmoteSetObjectId::VirtualId(VirtualId(user_id)) => {
				// check if there is a user with the provided id
				let (user, perms) = load_user_and_permissions(global, user_id).await?.ok_or(ApiError::NOT_FOUND)?;

				let user_connections = global
					.user_connection_by_user_id_loader()
					.load(user.id)
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.unwrap_or_default();

				let slots = perms.emote_set_slots_limit.unwrap_or(600);

				Ok(EmoteSet::virtual_set_for_user(user, user_connections, slots).await)
			}
		}
	}

	#[graphql(name = "emoteSetsByID")]
	async fn emote_sets_by_id<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		list: Vec<EmoteSetObjectId>,
	) -> Result<Vec<EmoteSet>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		if list.len() > 1000 {
			return Err(ApiError::new_const(StatusCode::BAD_REQUEST, "list too large"));
		}

		let mut set_ids = vec![];
		let mut virtual_user_ids = vec![];

		for id in list {
			match id {
				EmoteSetObjectId::Id(id) => set_ids.push(id),
				EmoteSetObjectId::VirtualId(VirtualId(user_id)) => virtual_user_ids.push(user_id),
			}
		}

		let mut emote_sets: Vec<_> = global
			.emote_set_by_id_loader()
			.load_many(set_ids)
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
			.into_values()
			.map(EmoteSet::from_db)
			.collect();

		// load users with ids for virtual sets
		let users = load_users_and_permissions(global, virtual_user_ids).await?;

		let user_connections = global
			.user_connection_by_user_id_loader()
			.load_many(users.keys().copied())
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		for (id, (user, perms)) in users {
			let slots = perms.emote_set_slots_limit.unwrap_or(600);
			let set =
				EmoteSet::virtual_set_for_user(user, user_connections.get(&id).cloned().unwrap_or_default(), slots).await;
			emote_sets.push(set);
		}

		Ok(emote_sets)
	}

	async fn named_emote_set<'ctx>(&self, ctx: &Context<'ctx>, name: EmoteSetName) -> Result<EmoteSet, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		match name {
			EmoteSetName::Global => {
				let global_config = global
					.global_config_loader()
					.load(())
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::INTERNAL_SERVER_ERROR)?;

				let global_emote_set = global_config.emote_set_ids.first().ok_or(ApiError::NOT_FOUND)?;

				let global_set = global
					.emote_set_by_id_loader()
					.load(*global_emote_set)
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::NOT_FOUND)?;

				Ok(EmoteSet::from_db(global_set))
			}
		}
	}
}

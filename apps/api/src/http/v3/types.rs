use bitflags::bitflags;
use shared::types::old::{ImageHost, UserModelPartial, UserStyle};
use ulid::Ulid;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct User {
	pub id: Ulid,
	#[serde(rename = "type", skip_serializing_if = "String::is_empty")]
	pub ty: String,
	pub username: String,
	pub display_name: String,
	pub created_at: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub avatar_url: Option<String>,
	#[serde(skip_serializing_if = "String::is_empty")]
	pub biography: String,
	pub style: UserStyle,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub emote_sets: Vec<EmoteSetPartial>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub editors: Vec<UserEditor>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub roles: Vec<Ulid>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub connections: Vec<UserConnection>,
}

impl From<crate::database::User> for User {
	fn from(value: crate::database::User) -> Self {
		Self {
			id: value.id,
			ty: "".to_string(),
			username: todo!(),
			display_name: todo!(),
			created_at: value.id.timestamp_ms(),
			avatar_url: None,
			biography: todo!(),
			style: UserStyle::default(),
			emote_sets: todo!(),
			editors: todo!(),
			roles: todo!(),
			connections: todo!(),
		}
	}
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct EmoteSetPartial {
	id: Ulid,
	name: String,
	flags: EmoteSetFlags,
	tags: Vec<String>,
	capacity: u32,
	#[serde(skip_serializing_if = "Option::is_none")]
	owner: Option<UserModelPartial>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct EmoteSetFlags(u32);

bitflags! {
	impl EmoteSetFlags: u32 {
		const IMMUTABLE = 1 << 0;
		const PRIVILEGED = 1 << 1;
		const PERSONAL = 1 << 2;
		const COMMERCIAL = 1 << 3;
	}
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct UserEditor {
	id: Ulid,
	permissions: i32,
	visible: bool,
	added_at: u64,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct UserConnection {
	id: Ulid,
	platform: String,
	username: String,
	display_name: String,
	linked_at: u64,
	emote_capacity: u32,
	emote_set_id: Option<Ulid>,
	emote_set: Option<EmoteSetPartial>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	presences: Vec<UserModelPartial>,
	#[serde(skip_serializing_if = "Option::is_none")]
	user: Option<User>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct Emote {
	pub id: Ulid,
	pub name: String,
	pub flags: EmoteFlags,
	pub tags: Vec<String>,
	pub lifecycle: EmoteLifecycle,
	pub state: Vec<EmoteVersionState>,
	pub listed: bool,
	pub animated: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub owner: Option<UserModelPartial>,
	pub host: ImageHost,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub versions: Vec<EmoteVersion>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct EmoteFlags(u32);

bitflags! {
	impl EmoteFlags: u32 {
		const PRIVATE = 1 << 0;
		const AUTHENTIC = 1 << 1;
		const ZERO_WIDTH = 1 << 8;

		const CONTENT_SEXUAL = 1 << 16;
		const CONTENT_EPILEPSY = 1 << 17;
		const CONTENT_EDGY = 1 << 18;
		const CONTENT_TWITCH_DISALLOWED = 1 << 24;
	}
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[repr(i32)]
pub enum EmoteLifecycle {
	Deleted = -1,
	Disabled = 2,
	Failed = -2,
	Live = 3,
	#[default]
	Pending = 0,
	Processing = 1,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct EmoteVersion {
	pub id: Ulid,
	pub name: String,
	pub description: String,
	pub lifecycle: EmoteLifecycle,
	pub state: Vec<EmoteVersionState>,
	pub listed: bool,
	pub animated: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub host: Option<ImageHost>,
	pub created_at: u64,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmoteVersionState {
	Listed,
	Personal,
	NoPersonal,
}

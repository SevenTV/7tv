use std::collections::HashMap;

use ulid::Ulid;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct InboxMessageModel {
    pub id: Ulid,
    pub kind: MessageKind,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author_id: Option<Ulid>,
    pub read: bool,
    #[serde(rename = "readAt")]
    pub read_at: Option<i64>,
    pub subject: String,
    pub content: String,
    pub important: bool,
    pub starred: bool,
    pub pinned: bool,
    pub placeholders: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct ModRequestMessageModel {
    pub id: Ulid,
    pub kind: MessageKind,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub author_id: Option<Ulid>,
    #[serde(rename = "targetKind")]
    pub target_kind: i32,
    #[serde(rename = "targetID")]
    pub target_id: Ulid,
    pub read: bool,
    pub wish: String,
    pub actor_country_name: String,
    pub actor_country_code: String,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageKind {
    #[default]
    EmoteComment,
    ModRequest,
    Inbox,
    News,
}

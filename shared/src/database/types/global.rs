use super::{EmoteSetId, RoleId};
use crate::database::{Collection, Id};

pub type GlobalConfigId = Id<GlobalConfig>;

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
	#[serde(rename = "_id", skip_serializing_if = "Id::is_nil")]
	pub id: GlobalConfigId,
	pub alerts: GlobalConfigAlerts,
	pub emote_set_ids: Vec<EmoteSetId>,
	pub role_ids: Vec<RoleId>,
}

impl Collection for GlobalConfig {
	const COLLECTION_NAME: &'static str = "global_config";
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfigAlerts {}

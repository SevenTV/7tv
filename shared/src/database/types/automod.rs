use super::UserId;
use crate::database::{Collection, Id};

pub type AutomodRuleId = Id<AutomodRule>;

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutomodRule {
	#[serde(rename = "_id")]
	pub id: AutomodRuleId,
	pub name: String,
	pub description: String,
	pub tags: Vec<String>,
	pub priority: i16,
	pub added_by: Option<UserId>,
	pub kind: AutomodRuleKind,
	pub enabled: bool,
	pub words: Vec<String>,
	pub allowed_words: Vec<String>,
	pub action: Option<AutomodRuleAction>,
}

impl Collection for AutomodRule {
	const COLLECTION_NAME: &'static str = "automod_rules";
}

#[derive(Debug, serde_repr::Serialize_repr, serde_repr::Deserialize_repr, Clone, Default)]
#[repr(u8)]
pub enum AutomodRuleKind {
	#[default]
	Normal = 0,
	Regex = 1,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
// Not sure what the difference between these two is
pub enum AutomodRuleAction {
	Timeout(i64),
	Ban(i64),
}

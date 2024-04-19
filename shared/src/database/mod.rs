use crate::config::DatabaseConfig;

mod id;
mod types;

pub use id::Id;
pub use types::*;

pub async fn setup_database(config: &DatabaseConfig) -> anyhow::Result<mongodb::Client> {
	let options = mongodb::options::ClientOptions::parse(&config.uri).await?;
	Ok(mongodb::Client::with_options(options)?)
}

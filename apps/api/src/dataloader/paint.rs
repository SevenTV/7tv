use std::sync::Arc;

use scuffle_utils::dataloader::{DataLoader, Loader, LoaderOutput};
use ulid::Ulid;
use shared::database::Paint;

pub struct PaintByIdLoader {
	db: Arc<scuffle_utils::database::Pool>,
}

impl PaintByIdLoader {
	pub fn new(db: Arc<scuffle_utils::database::Pool>) -> DataLoader<Self> {
		DataLoader::new(Self { db })
	}
}

impl Loader for PaintByIdLoader {
	type Error = ();
	type Key = Ulid;
	type Value = Paint;

	async fn load(&self, keys: &[Self::Key]) -> LoaderOutput<Self> {
		let results: Vec<Self::Value> = scuffle_utils::database::query("SELECT * FROM paints WHERE id = ANY($1)")
			.bind(keys)
			.build_query_as()
			.fetch_all(&self.db)
			.await
			.map_err(|e| {
				tracing::error!(err = %e, "failed to fetch paints by id");
			})?;

		Ok(results.into_iter().map(|r| (r.id, r)).collect())
	}
}

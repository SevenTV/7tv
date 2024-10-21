use std::future::IntoFuture;
use std::sync::Arc;

use async_graphql::{Context, Object};
use futures::{TryFutureExt, TryStreamExt};
use mongodb::bson::doc;
use shared::database::badge::Badge;
use shared::database::paint::Paint;
use shared::database::queries::filter;
use shared::database::MongoCollection;
use shared::old_types::cosmetic::{CosmeticBadgeModel, CosmeticPaintModel};
use shared::old_types::object_id::GqlObjectId;

use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};

// https://github.com/SevenTV/API/blob/main/internal/api/gql/v3/schema/cosmetics.gql

#[derive(Default)]
pub struct CosmeticsQuery;

#[derive(Debug, Clone, Default, async_graphql::SimpleObject)]
#[graphql(name = "CosmeticsQuery", rename_fields = "snake_case")]
pub struct CosmeticsQueryResponse {
	paints: Vec<CosmeticPaintModel>,
	badges: Vec<CosmeticBadgeModel>,
}

#[Object(name = "CosmeticsRootQuery", rename_fields = "camelCase", rename_args = "snake_case")]
impl CosmeticsQuery {
	async fn cosmetics<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		#[graphql(validator(max_items = 600))] list: Option<Vec<GqlObjectId>>,
	) -> Result<CosmeticsQueryResponse, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let list = list.unwrap_or_default();

		if list.is_empty() {
			// return all cosmetics when empty list is provided
			let mut paints: Vec<_> = Paint::collection(&global.db)
				.find(filter::filter!(Paint {}))
				.into_future()
				.and_then(|f| f.try_collect::<Vec<Paint>>())
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to query paints");
					ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to query paints")
				})?
				.into_iter()
				.map(|p| CosmeticPaintModel::from_db(p, &global.config.api.cdn_origin))
				.collect();

			paints.sort_by(|a, b| a.id.cmp(&b.id));

			let mut badges: Vec<_> = Badge::collection(&global.db)
				.find(filter::filter!(Badge {}))
				.into_future()
				.and_then(|f| f.try_collect::<Vec<Badge>>())
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to query badges");
					ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to query badges")
				})?
				.into_iter()
				.map(|b: Badge| CosmeticBadgeModel::from_db(b, &global.config.api.cdn_origin))
				.collect();

			badges.sort_by(|a, b| a.id.cmp(&b.id));

			Ok(CosmeticsQueryResponse { paints, badges })
		} else {
			let mut paints: Vec<_> = global
				.paint_by_id_loader
				.load_many(list.clone().into_iter().map(|id| id.id()))
				.await
				.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load paints"))?
				.into_values()
				.map(|p| CosmeticPaintModel::from_db(p, &global.config.api.cdn_origin))
				.collect();

			paints.sort_by(|a, b| a.id.cmp(&b.id));

			let mut badges: Vec<_> = global
				.badge_by_id_loader
				.load_many(list.into_iter().map(|id| id.id()))
				.await
				.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load badges"))?
				.into_values()
				.map(|b| CosmeticBadgeModel::from_db(b, &global.config.api.cdn_origin))
				.collect();

			badges.sort_by(|a, b| a.id.cmp(&b.id));

			Ok(CosmeticsQueryResponse { paints, badges })
		}
	}
}

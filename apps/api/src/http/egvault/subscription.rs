use std::sync::Arc;

use axum::extract::State;
use axum::{Extension, Json};
use shared::database::product::subscription::{ProviderSubscriptionId, SubscriptionPeriodCreatedBy, SubscriptionState};
use shared::database::product::{SubscriptionProduct, SubscriptionProductKind, SubscriptionProductVariant};

use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};
use crate::http::extract::Path;
use crate::http::middleware::session::Session;
use crate::http::v3::rest::types::{self, SubscriptionCycleUnit};
use crate::http::v3::rest::users::TargetUser;
use crate::sub_refresh_job;

#[derive(Debug, serde::Serialize)]
pub struct SubscriptionResponse {
	pub active: bool,
	pub age: u32,
	pub months: u32,
	pub renew: bool,
	/// Date of the next renewal
	pub end_at: Option<chrono::DateTime<chrono::Utc>>,
	pub subscription: Option<types::Subscription>,
}

pub async fn subscription(
	State(global): State<Arc<Global>>,
	Path(target): Path<TargetUser>,
	Extension(session): Extension<Session>,
) -> Result<Json<SubscriptionResponse>, ApiError> {
	let user = match target {
		TargetUser::Me => session.user()?.id,
		TargetUser::Other(id) => id,
	};

	let periods: Vec<_> = global
		.subscription_periods_by_user_id_loader
		.load(user)
		.await
		.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription periods"))?
		.unwrap_or_default();

	let Some(active_period) = periods
		.iter()
		.find(|p| p.start < chrono::Utc::now() && p.end > chrono::Utc::now())
		.cloned()
	else {
		return Ok(Json(SubscriptionResponse {
			active: false,
			age: 0,
			months: 0,
			renew: false,
			end_at: None,
			subscription: None,
		}));
	};

	let subscription = global
		.subscription_by_id_loader
		.load(active_period.subscription_id)
		.await
		.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription"))?
		.ok_or_else(|| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription"))?;

	let periods: Vec<_> = periods
		.into_iter()
		.filter(|p| p.subscription_id == active_period.subscription_id)
		.collect();

	let product: SubscriptionProduct = global
		.subscription_product_by_id_loader
		.load(subscription.id.product_id)
		.await
		.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription product"))?
		.ok_or_else(|| ApiError::internal_server_error(ApiErrorCode::LoadError, "could not find subscription product"))?;

	let age = sub_refresh_job::SubAge::new(&periods);

	let provider = active_period.provider_id.as_ref().map(|id| match id {
		ProviderSubscriptionId::Stripe(_) => types::Provider::Stripe,
		ProviderSubscriptionId::Paypal(_) => types::Provider::Paypal,
	});

	let customer_id = active_period.gifted_by.unwrap_or(active_period.subscription_id.user_id);

	let internal = matches!(active_period.created_by, SubscriptionPeriodCreatedBy::System { .. });

	let unit = product.variants.iter().find(|v| v.id == active_period.product_id);

	let started_at = periods
		.iter()
		.min_by_key(|p| p.start)
		.map(|p| p.start)
		.unwrap_or(active_period.start);

	let end_at = periods
		.iter()
		.max_by_key(|p| p.end)
		.map(|p| p.end)
		.unwrap_or(active_period.end);

	let trial_end = active_period.is_trial.then_some(active_period.end);

	let renew = subscription.state != SubscriptionState::CancelAtEnd;

	Ok(Json(SubscriptionResponse {
		active: true,
		age: age.days as u32,
		months: age.months as u32,
		renew,
		end_at: Some(end_at),
		subscription: Some(types::Subscription {
			id: active_period.id,
			provider,
			product_id: product.id,
			plan: active_period.product_id,
			seats: 1,
			subscriber_id: subscription.id.user_id,
			customer_id,
			started_at,
			ended_at: (subscription.state == SubscriptionState::Ended).then_some(end_at),
			cycle: types::SubscriptionCycle {
				timestamp: active_period.end,
				unit: unit.map(|unit| match unit {
					SubscriptionProductVariant {
						kind: SubscriptionProductKind::Monthly,
						..
					} => SubscriptionCycleUnit::Month,
					SubscriptionProductVariant {
						kind: SubscriptionProductKind::Yearly,
						..
					} => SubscriptionCycleUnit::Year,
				}),
				value: 1,
				status: subscription.state.into(),
				internal,
				pending: false,
				trial_end,
			},
			renew,
		}),
	}))
}

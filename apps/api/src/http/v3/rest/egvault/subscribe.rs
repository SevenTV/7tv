use std::{collections::HashMap, sync::Arc};

use axum::{extract::State, http::StatusCode, Extension, Json};
use shared::database::{
	product::SubscriptionProduct, product::SubscriptionProductKind, queries::filter, user::UserId, MongoCollection,
};

use crate::{
	global::Global,
	http::{error::ApiError, extract::Query, middleware::auth::AuthSession},
};

use super::{create_checkout_session_params, find_customer};

#[derive(Debug, serde::Deserialize)]
pub struct SubscribeQuery {
	renew_interval: SubscriptionRenewInterval,
	/// only "stripe" allowed
	payment_method: String,
	/// always true
    #[serde(rename = "next")]
	_next: bool,
	gift_for: Option<UserId>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SubscribeBody {
	prefill: Prefill,
}

#[derive(Debug, serde::Deserialize)]
pub struct Prefill {
	// first_name: String,
	// last_name: String,
	email: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionRenewInterval {
	Monthly,
	Yearly,
}

impl From<SubscriptionRenewInterval> for SubscriptionProductKind {
	fn from(value: SubscriptionRenewInterval) -> Self {
		match value {
			SubscriptionRenewInterval::Monthly => Self::Monthly,
			SubscriptionRenewInterval::Yearly => Self::Yearly,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct SubscribeResponse {
	/// Url that the website will open in a new tab
	url: String,
	/// The user id of the user that receives the subscription
	user_id: UserId,
}

pub async fn subscribe(
	State(global): State<Arc<Global>>,
	Query(query): Query<SubscribeQuery>,
	auth_session: Option<Extension<AuthSession>>,
	Json(body): Json<SubscribeBody>,
) -> Result<Json<SubscribeResponse>, ApiError> {
	let auth_session = auth_session.ok_or(ApiError::UNAUTHORIZED)?;

	if query.payment_method != "stripe" {
		return Err(ApiError::new_const(StatusCode::BAD_REQUEST, "payment method not supported"));
	}

	let product: SubscriptionProduct = SubscriptionProduct::collection(&global.db)
		.find_one(filter::filter! {
			SubscriptionProduct {
				#[query(serde)]
				kind: SubscriptionProductKind::from(query.renew_interval),
			}
		})
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to find subscription product");
			ApiError::INTERNAL_SERVER_ERROR
		})?
		.ok_or(ApiError::new_const(StatusCode::NOT_FOUND, "subscription product not found"))?;

	let customer_id = match auth_session.user(&global).await?.stripe_customer_id.clone() {
		Some(id) => Some(id),
		None => find_customer(&global, auth_session.user_id()).await?,
	};

	let paying_user = auth_session.user_id();

	let mut params = create_checkout_session_params(&global, customer_id, Some(&body.prefill.email), Some(&product.id)).await;

	let receiving_user = if let Some(gift_for) = query.gift_for {
		let receiving_user = global
			.user_loader
			.load_fast(&global, gift_for)
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or(ApiError::new_const(StatusCode::NOT_FOUND, "user not found"))?;

		if receiving_user.computed.is_subscribed {
			return Err(ApiError::new_const(StatusCode::BAD_REQUEST, "user is already subscribed"));
		}

		let mut metadata: HashMap<_, _> = [
			("USER_ID".to_string(), receiving_user.id.to_string()),
			("CUSTOMER_ID".to_string(), paying_user.to_string()),
		]
		.into();

		params.mode = Some(stripe::CheckoutSessionMode::Payment);
		params.payment_intent_data = Some(stripe::CreateCheckoutSessionPaymentIntentData {
			description: Some("Gift subscription payment".to_string()),
			metadata: Some(metadata.clone()),
			..Default::default()
		});

		metadata.insert("IS_GIFT".to_string(), "true".to_string());
		metadata.insert("PRODUCT_ID".to_string(), product.id.to_string());
		metadata.insert(
			"PERIOD_DURATION_MONTHS".to_string(),
			product.kind.period_duration_months().to_string(),
		);

		params.metadata = Some(metadata);

		receiving_user.id
	} else {
		if auth_session.user(&global).await?.computed.is_subscribed {
			return Err(ApiError::new_const(StatusCode::BAD_REQUEST, "user is already subscribed"));
		}

		params.mode = Some(stripe::CheckoutSessionMode::Subscription);
		params.subscription_data = Some(stripe::CreateCheckoutSessionSubscriptionData {
			metadata: Some([("USER_ID".to_string(), auth_session.user_id().to_string())].into()),
			..Default::default()
		});

		auth_session.user_id()
	};

	let session_url = stripe::CheckoutSession::create(&global.stripe_client, params)
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to create checkout session");
			ApiError::INTERNAL_SERVER_ERROR
		})?
		.url
		.ok_or(ApiError::INTERNAL_SERVER_ERROR)?;

	Ok(Json(SubscribeResponse {
		url: session_url,
		user_id: receiving_user,
	}))
}

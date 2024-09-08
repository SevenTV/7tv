use std::ops::Deref;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::{Extension, Json};

use super::find_or_create_customer;
use super::metadata::{CheckoutSessionMetadata, StripeMetadata};
use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::extract::Path;
use crate::http::middleware::auth::AuthSession;
use crate::http::v3::rest::users::TargetUser;

#[derive(Debug, serde::Deserialize)]
pub struct PaymentMethodQuery {
	/// always true
	#[serde(rename = "next")]
	_next: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct PaymentMethodResponse {
	/// Url that the website will open in a new tab
	url: String,
}

pub async fn payment_method(
	State(global): State<Arc<Global>>,
	Path(target): Path<TargetUser>,
	Query(_query): Query<PaymentMethodQuery>,
	Extension(ip): Extension<std::net::IpAddr>,
	auth_session: Option<AuthSession>,
) -> Result<Json<PaymentMethodResponse>, ApiError> {
	let auth_session = auth_session.ok_or(ApiError::UNAUTHORIZED)?;

	let user = match target {
		TargetUser::Me => auth_session.user_id(),
		TargetUser::Other(id) => id,
	};

	if user != auth_session.user_id() {
		// TODO: allow with certain permissions
		return Err(ApiError::FORBIDDEN);
	}

	let stripe_tx = global.stripe_client.safe().await;

	let customer_id = match auth_session.user(&global).await?.stripe_customer_id.clone() {
		Some(id) => id,
		None => find_or_create_customer(&global, stripe_tx.client(0).await, auth_session.user_id(), None).await?,
	};

	let success_url = format!("{}/subscribe", global.config.api.website_origin);
	let cancel_url = format!("{}/subscribe", global.config.api.website_origin);

	let mut currency = stripe::Currency::EUR;

	if let Some(country_code) = global.geoip().and_then(|g| g.lookup(ip)).and_then(|c| c.iso_code) {
		if let Ok(Some(global)) = global.global_config_loader.load(()).await {
			if let Some(currency_override) = global.country_currency_overrides.get(country_code) {
				currency = *currency_override;
			}
		}
	}

	let params = stripe::CreateCheckoutSession {
		line_items: None,
		mode: Some(stripe::CheckoutSessionMode::Setup),
		customer_update: Some(stripe::CreateCheckoutSessionCustomerUpdate {
			address: Some(stripe::CreateCheckoutSessionCustomerUpdateAddress::Auto),
			..Default::default()
		}),
		currency: Some(currency),
		customer: Some(customer_id.into()),
		// expire the session 4 hours from now so we can restore unused redeem codes in the checkout.session.expired handler
		expires_at: Some((chrono::Utc::now() + chrono::Duration::hours(4)).timestamp()),
		success_url: Some(&success_url),
		cancel_url: Some(&cancel_url),
		metadata: Some(CheckoutSessionMetadata::Setup.to_stripe()),
		..Default::default()
	};

	let url = stripe::CheckoutSession::create(stripe_tx.client(1).await.deref(), params)
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to create checkout session");
			ApiError::INTERNAL_SERVER_ERROR
		})?
		.url
		.ok_or(ApiError::INTERNAL_SERVER_ERROR)?;

	Ok(Json(PaymentMethodResponse { url }))
}

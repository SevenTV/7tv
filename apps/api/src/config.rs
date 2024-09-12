use std::net::SocketAddr;
use std::path::PathBuf;

use scuffle_foundations::bootstrap::{Bootstrap, RuntimeSettings};
use scuffle_foundations::settings::auto_settings;
use scuffle_foundations::telemetry::settings::TelemetrySettings;
use shared::config::{ClickhouseConfig, DatabaseConfig, ImageProcessorConfig, NatsConfig, RedisConfig, TypesenseConfig};
use shared::ip::GeoIpConfig;

#[auto_settings]
#[serde(default)]
pub struct Api {
	/// http options
	#[settings(default = SocketAddr::from(([0, 0, 0, 0], 8080)))]
	pub bind: SocketAddr,
	/// worker count
	#[settings(default = 1)]
	pub workers: usize,
	/// website origin
	#[settings(default = "https://7tv.app".parse().unwrap())]
	pub website_origin: url::Url,
	/// cdn base url
	#[settings(default = "https://cdn.7tv.app/".parse().unwrap())]
	pub cdn_origin: url::Url,
	/// public domain
	#[settings(default = "7tv.io".into())]
	pub domain: String,
	/// base url
	#[settings(default = "https://7tv.io".parse().unwrap())]
	pub api_origin: url::Url,
	/// connection config
	pub connections: ConnectionsConfig,
	/// jwt config
	pub jwt: JwtConfig,
	/// image processor config
	pub image_processor: ImageProcessorConfig,
	/// Event API nats prefix
	#[settings(default = "api.events".into())]
	pub nats_event_subject: String,
	/// Stripe config
	pub stripe: StripeConfig,
	/// PayPal config
	pub paypal: PayPalConfig,
	/// GeoIP config
	pub geoip: Option<GeoIpConfig>,
	/// IP Header config
	pub incoming_request: IncomingRequestConfig,
	/// Redis config
	pub redis: RedisConfig,
}

#[auto_settings]
#[serde(default)]
pub struct IncomingRequestConfig {
	/// The IP header to use for incoming requests
	pub ip_header: Option<String>,
	/// A set of trusted proxies that we should use for incoming requests
	pub trusteded_proxies: Vec<ipnet::IpNet>,
}

#[auto_settings]
#[serde(default)]
pub struct JwtConfig {
	/// JWT secret
	#[settings(default = "seventv-api".into())]
	pub secret: String,

	/// JWT issuer
	#[settings(default = "seventv-api".into())]
	pub issuer: String,
}

#[auto_settings]
#[serde(default)]
pub struct ConnectionsConfig {
	/// Twitch connection
	pub twitch: ConnectionConfig,
	/// Discord connection
	pub discord: ConnectionConfig,
	/// Google connection
	pub google: ConnectionConfig,
}

#[auto_settings]
#[serde(default)]
pub struct ConnectionConfig {
	/// If login with this connection is enabled
	#[settings(default = false)]
	pub enabled: bool,
	/// Client ID
	#[settings(default = "client_id".into())]
	pub client_id: String,
	/// Client Secret
	#[settings(default = "client_secret".into())]
	pub client_secret: String,
}

#[auto_settings]
#[serde(default)]
pub struct ChangeStreamConfig {
	/// Change Stream Prefix
	#[settings(default = "seventv".into())]
	pub prefix: String,

	/// The number of pending acks to buffer
	#[settings(default = 1000)]
	pub back_pressure_limit: usize,
}

#[auto_settings]
#[serde(default)]
pub struct StripeConfig {
	/// Stripe API key
	#[settings(default = "sk_test_123".into())]
	pub api_key: String,
	/// Stripe webhook secret
	#[settings(default = "whsec_test".into())]
	pub webhook_secret: String,
	/// Stripe concurrent requests
	#[settings(default = 50)]
	pub concurrent_requests: usize,
}

#[auto_settings]
#[serde(default)]
pub struct PayPalConfig {
	/// Paypal api key
	/// Needed to fetch subscriptions
	#[settings(default = "api_key".into())]
	pub api_key: String,
	/// PayPal webhook id
	#[settings(default = "webhook_id".into())]
	pub webhook_id: String,
}

#[auto_settings]
#[serde(default)]
pub struct Config {
	/// Export GQL schema
	#[settings(default = None)]
	pub export_schema_path: Option<PathBuf>,

	/// API configuration
	pub api: Api,

	/// Database configuration
	pub database: DatabaseConfig,

	/// Clickhouse configuration
	pub clickhouse: ClickhouseConfig,

	/// NATs configuration
	pub nats: NatsConfig,

	/// Typesense configuration
	pub typesense: TypesenseConfig,

	/// Telemetry configuration
	pub telemetry: TelemetrySettings,

	/// Runtime configuration
	pub runtime: RuntimeSettings,
}

impl Bootstrap for Config {
	type Settings = Self;

	fn telemetry_config(&self) -> Option<TelemetrySettings> {
		Some(self.telemetry.clone())
	}

	fn runtime_mode(&self) -> RuntimeSettings {
		self.runtime.clone()
	}
}

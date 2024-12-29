use std::sync::Arc;

use anyhow::Context as _;
use scuffle_bootstrap_telemetry::opentelemetry;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::metrics::SdkMeterProvider;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::Resource;
use scuffle_metrics::opentelemetry::KeyValue;
use shared::database::mongodb;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::Config;

pub struct Global {
	pub db: mongodb::Database,
	pub nats: async_nats::Client,
	pub config: Config,
	metrics: scuffle_bootstrap_telemetry::prometheus_client::registry::Registry,
}

impl scuffle_bootstrap::global::Global for Global {
	type Config = Config;

	fn pre_init() -> anyhow::Result<()> {
		rustls::crypto::aws_lc_rs::default_provider().install_default().ok();
		Ok(())
	}

	async fn init(config: Config) -> anyhow::Result<Arc<Self>> {
		let mut metrics = scuffle_bootstrap_telemetry::prometheus_client::registry::Registry::default();

		let exporter = scuffle_metrics::prometheus::exporter().build();
		metrics.register_collector(exporter.collector());

		opentelemetry::global::set_meter_provider(
			SdkMeterProvider::builder()
				.with_resource(Resource::new(vec![KeyValue::new("service.name", env!("CARGO_BIN_NAME"))]))
				.with_reader(exporter)
				.build(),
		);

		tracing_subscriber::registry()
			.with(
				tracing_subscriber::fmt::layer()
					.with_file(true)
					.with_line_number(true)
					.with_filter(
						EnvFilter::builder()
							.with_default_directive(LevelFilter::INFO.into())
							.parse_lossy(&config.level),
					),
			)
			.init();

		let (nats, _) = shared::nats::setup_nats("discord-bot", &config.nats)
			.await
			.context("nats connect")?;

		tracing::info!("connected to nats");

		let mongo = shared::database::setup_and_init_database(&config.database)
			.await
			.context("database setup")?;

		tracing::info!("connected to mongo");

		let db = mongo
			.default_database()
			.ok_or_else(|| anyhow::anyhow!("No default database"))?;

		Ok(Arc::new(Self {
			db,
			nats,
			config,
			metrics,
		}))
	}
}

impl scuffle_signal::SignalConfig for Global {
	async fn on_shutdown(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("shutting down discord bot");

		Ok(())
	}
}

impl scuffle_bootstrap_telemetry::TelemetryConfig for Global {
	async fn health_check(&self) -> Result<(), anyhow::Error> {
		tracing::debug!("running health check");

		if !matches!(self.nats.connection_state(), async_nats::connection::State::Connected) {
			anyhow::bail!("nats not connected");
		}

		Ok(())
	}

	fn bind_address(&self) -> Option<std::net::SocketAddr> {
		self.config.metrics_bind_address
	}

	fn prometheus_metrics_registry(&self) -> Option<&scuffle_bootstrap_telemetry::prometheus_client::registry::Registry> {
		Some(&self.metrics)
	}
}

use std::sync::Arc;

use anyhow::Context;
use scuffle_bootstrap_telemetry::opentelemetry;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::metrics::SdkMeterProvider;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::Resource;
use scuffle_metrics::opentelemetry::KeyValue;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::cache;
use crate::config::Config;

pub struct Global {
	pub config: Config,
	pub cache: cache::Cache,
	pub jetstream: async_nats::jetstream::Context,
	pub metrics: scuffle_bootstrap_telemetry::prometheus::Registry,
}

impl scuffle_bootstrap::global::Global for Global {
	type Config = Config;

	fn pre_init() -> anyhow::Result<()> {
		rustls::crypto::aws_lc_rs::default_provider().install_default().ok();
		Ok(())
	}

	async fn init(config: Self::Config) -> anyhow::Result<Arc<Self>> {
		let metrics = scuffle_bootstrap_telemetry::prometheus::Registry::new();

		opentelemetry::global::set_meter_provider(
			SdkMeterProvider::builder()
				.with_resource(Resource::new(vec![KeyValue::new("service.name", env!("CARGO_BIN_NAME"))]))
				.with_reader(
					scuffle_metrics::prometheus::exporter()
						.with_registry(metrics.clone())
						.build()
						.context("prometheus metrics exporter")?,
				)
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

		tracing::info!("starting cdn");

		let (_, jetstream) = shared::nats::setup_nats(&config.pod.name, &config.nats)
			.await
			.context("nats")?;

		Ok(Arc::new(Self {
			cache: cache::Cache::new(&config.cdn),
			config,
			jetstream,
			metrics,
		}))
	}
}

impl scuffle_bootstrap_telemetry::TelemetryConfig for Global {
	async fn health_check(&self) -> Result<(), anyhow::Error> {
		tracing::debug!("running health check");
		Ok(())
	}

	fn bind_address(&self) -> Option<std::net::SocketAddr> {
		self.config.metrics_bind_address
	}

	fn prometheus_metrics_registry(&self) -> Option<&scuffle_bootstrap_telemetry::prometheus::Registry> {
		Some(&self.metrics)
	}
}

impl scuffle_signal::SignalConfig for Global {
	async fn on_shutdown(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("shutting down cdn");
		Ok(())
	}
}

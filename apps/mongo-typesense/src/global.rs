use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

use anyhow::Context as _;
use scuffle_batching::{Batcher, DataLoader};
use scuffle_bootstrap_telemetry::opentelemetry;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::metrics::SdkMeterProvider;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::Resource;
use scuffle_metrics::opentelemetry::KeyValue;
use shared::clickhouse::emote_stat::EmoteStat;
use shared::database::entitlement_edge::{EntitlementEdgeInboundLoader, EntitlementEdgeOutboundLoader};
use shared::database::updater::MongoUpdater;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
use typesense_rs::apis::Api;

use crate::batcher::clickhouse::ClickhouseInsert;
use crate::batcher::CollectionBatcher;
use crate::config::Config;
use crate::types::*;

pub struct Global {
	pub nats: async_nats::Client,
	pub jetstream: async_nats::jetstream::Context,
	pub database: mongodb::Database,
	pub config: Config,
	pub typesense: Arc<typesense_rs::apis::ApiClient>,
	pub event_batcher: CollectionBatcher<mongo::StoredEvent>,
	pub user_batcher: CollectionBatcher<mongo::User>,
	pub badge_batcher: CollectionBatcher<mongo::Badge>,
	pub emote_batcher: CollectionBatcher<mongo::Emote>,
	pub emote_moderation_request_batcher: CollectionBatcher<mongo::EmoteModerationRequest>,
	pub emote_set_batcher: CollectionBatcher<mongo::EmoteSet>,
	pub paint_batcher: CollectionBatcher<mongo::Paint>,
	pub role_batcher: CollectionBatcher<mongo::Role>,
	pub ticket_batcher: CollectionBatcher<mongo::Ticket>,
	pub ticket_message_batcher: CollectionBatcher<mongo::TicketMessage>,
	pub redeem_code_batcher: CollectionBatcher<mongo::RedeemCode>,
	pub special_event_batcher: CollectionBatcher<mongo::SpecialEvent>,
	pub invoice_batcher: CollectionBatcher<mongo::Invoice>,
	pub product_batcher: CollectionBatcher<mongo::Product>,
	pub subscription_period_batcher: CollectionBatcher<mongo::SubscriptionPeriod>,
	pub user_ban_batcher: CollectionBatcher<mongo::UserBan>,
	pub user_editor_batcher: CollectionBatcher<mongo::UserEditor>,
	pub entitlement_inbound_loader: DataLoader<EntitlementEdgeInboundLoader>,
	pub entitlement_outbound_loader: DataLoader<EntitlementEdgeOutboundLoader>,
	pub emote_stats_batcher: Batcher<ClickhouseInsert<EmoteStat>>,
	pub subscription_product_batcher: CollectionBatcher<mongo::SubscriptionProduct>,
	pub subscription_batcher: CollectionBatcher<mongo::Subscription>,
	pub updater: MongoUpdater,
	is_healthy: AtomicBool,
	request_count: AtomicUsize,
	health_state: tokio::sync::Mutex<HealthCheckState>,
	semaphore: Arc<tokio::sync::Semaphore>,
	metrics: scuffle_bootstrap_telemetry::prometheus_client::registry::Registry,
}

#[derive(Debug, Default)]
struct HealthCheckState {
	nats_healthy: bool,
	db_healthy: bool,
	typesense_healthy: bool,
	last_check: Option<tokio::time::Instant>,
}

impl scuffle_bootstrap::global::Global for Global {
	type Config = Config;

	fn pre_init() -> anyhow::Result<()> {
		rustls::crypto::aws_lc_rs::default_provider().install_default().ok();
		Ok(())
	}

	async fn init(config: Config) -> anyhow::Result<Arc<Self>> {
		let mut prometheus_registry = scuffle_bootstrap_telemetry::prometheus_client::registry::Registry::default();

		let exporter = scuffle_metrics::prometheus::exporter().build();
		prometheus_registry.register_collector(exporter.collector());

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

		let (nats, jetstream) = shared::nats::setup_nats("event-api", &config.nats)
			.await
			.context("nats connect")?;

		let database = mongodb::Client::with_uri_str(&config.database.uri)
			.await
			.context("mongo connect")?
			.default_database()
			.ok_or_else(|| anyhow::anyhow!("no default database"))?;

		let typesense = Arc::new(typesense_rs::apis::ApiClient::new(Arc::new(
			typesense_rs::apis::configuration::Configuration {
				base_path: config.typesense.uri.clone(),
				api_key: config
					.typesense
					.api_key
					.clone()
					.map(|key| typesense_rs::apis::configuration::ApiKey { key, prefix: None }),
				..Default::default()
			},
		)));

		let clickhouse = shared::clickhouse::init_clickhouse(&config.clickhouse).await?;

		Ok(Arc::new(Self {
			nats,
			jetstream,
			event_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			user_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			badge_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			emote_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			emote_moderation_request_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			emote_set_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			paint_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			role_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			ticket_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			ticket_message_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			redeem_code_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			product_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			subscription_period_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			user_ban_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			user_editor_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			special_event_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			invoice_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			entitlement_inbound_loader: EntitlementEdgeInboundLoader::new(database.clone()),
			entitlement_outbound_loader: EntitlementEdgeOutboundLoader::new(database.clone()),
			subscription_product_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			subscription_batcher: CollectionBatcher::new(database.clone(), typesense.clone()),
			updater: MongoUpdater::new(database.clone(), 500, 5_000, std::time::Duration::from_millis(300)),
			typesense,
			database,
			is_healthy: AtomicBool::new(false),
			request_count: AtomicUsize::new(0),
			health_state: tokio::sync::Mutex::new(HealthCheckState::default()),
			semaphore: Arc::new(tokio::sync::Semaphore::new(config.triggers.typesense_concurrency.max(1))),
			emote_stats_batcher: ClickhouseInsert::new(clickhouse),
			config,
			metrics: prometheus_registry,
		}))
	}
}

impl Global {
	pub fn report_error(&self) {
		self.is_healthy.store(false, std::sync::atomic::Ordering::Relaxed);
	}

	pub fn is_healthy(&self) -> bool {
		self.is_healthy.load(std::sync::atomic::Ordering::Relaxed)
	}

	pub async fn wait_healthy(&self) -> bool {
		if self.is_healthy() {
			return true;
		}

		self.do_health_check().await
	}

	async fn do_health_check(&self) -> bool {
		let mut state = self.health_state.lock().await;
		if state
			.last_check
			.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(5))
		{
			return state.nats_healthy && state.db_healthy && state.typesense_healthy;
		}

		tracing::debug!("running health check");

		state.nats_healthy = matches!(self.nats.connection_state(), async_nats::connection::State::Connected);
		if !state.nats_healthy {
			tracing::error!("nats not healthy");
		}

		state.db_healthy = match self.database.run_command(bson::doc! { "ping": 1 }).await {
			Ok(_) => true,
			Err(e) => {
				tracing::error!("mongo not healthy: {e}");
				false
			}
		};
		state.typesense_healthy = match self.typesense.health_api().health().await {
			Ok(r) => {
				if r.ok {
					true
				} else {
					tracing::error!("typesense not healthy");
					false
				}
			}
			Err(e) => {
				tracing::error!("typesense not healthy: {e}");
				false
			}
		};
		state.last_check = Some(tokio::time::Instant::now());

		self.is_healthy.store(
			state.nats_healthy && state.db_healthy && state.typesense_healthy,
			std::sync::atomic::Ordering::Relaxed,
		);

		state.nats_healthy && state.db_healthy && state.typesense_healthy
	}

	pub async fn aquire_ticket(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
		while !self.wait_healthy().await {
			tracing::warn!("waiting for mongo, typesense, and nats to be healthy");
			tokio::time::sleep(std::time::Duration::from_secs(5)).await;
		}

		self.semaphore.clone().acquire_owned().await.ok()
	}

	pub fn incr_request_count(&self) {
		self.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
	}

	pub fn request_count(&self) -> usize {
		self.request_count.load(std::sync::atomic::Ordering::Relaxed)
	}

	pub async fn reindex(&self) {
		macro_rules! reindex_collection {
			($($collection:ty),*$(,)?) => {
				{
					[
						$(
							shared::database::updater::MongoReq::update::<$collection>(
								shared::database::queries::filter::filter! {
									$collection {
										search_updated_at: &None,
									}
								},
								shared::database::queries::update::update! {
									#[query(set)]
									$collection {
										updated_at: chrono::Utc::now(),
									}
								},
								true,
							),
						)*
					]
				}
			}
		}

		for result in self
			.updater
			.bulk(reindex_collection! {
				crate::types::mongo::RedeemCode,
				crate::types::mongo::SpecialEvent,
				crate::types::mongo::Invoice,
				crate::types::mongo::Product,
				crate::types::mongo::SubscriptionProduct,
				crate::types::mongo::SubscriptionPeriod,
				crate::types::mongo::UserBan,
				crate::types::mongo::UserEditor,
				crate::types::mongo::User,
				crate::types::mongo::StoredEvent,
				crate::types::mongo::Badge,
				crate::types::mongo::EmoteModerationRequest,
				crate::types::mongo::EmoteSet,
				crate::types::mongo::Emote,
				crate::types::mongo::Paint,
				crate::types::mongo::Role,
				crate::types::mongo::Ticket,
				crate::types::mongo::TicketMessage,
				crate::types::mongo::Subscription,
			})
			.await
		{
			if let Err(e) = result {
				tracing::error!("failed to reindex: {e}");
			}
		}
	}
}

impl scuffle_bootstrap_telemetry::TelemetryConfig for Global {
	fn bind_address(&self) -> Option<std::net::SocketAddr> {
		self.config.metrics_bind_address
	}

	fn prometheus_metrics_registry(&self) -> Option<&scuffle_bootstrap_telemetry::prometheus_client::registry::Registry> {
		Some(&self.metrics)
	}

	async fn health_check(&self) -> Result<(), anyhow::Error> {
		if !self.do_health_check().await {
			anyhow::bail!("health check failed");
		}

		Ok(())
	}
}

impl scuffle_signal::SignalConfig for Global {
	async fn on_shutdown(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("shutting down");
		Ok(())
	}
}

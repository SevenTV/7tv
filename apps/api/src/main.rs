use std::sync::Arc;

use cap::Cap;
use scuffle_utils::context::Context;
use scuffle_utils::prelude::FutureTimeout;
use tokio::signal::unix::SignalKind;

mod config;
mod global;
mod metrics;

#[global_allocator]
static ALLOCATOR: Cap<tikv_jemallocator::Jemalloc> = Cap::new(tikv_jemallocator::Jemalloc, usize::max_value());

#[tokio::main]
async fn main() {
	let config = shared::config::parse(true, Some("config".into())).expect("failed to parse config");
	shared::logging::init(&config.logging.level, config.logging.mode).expect("failed to initialize logging");

	if let Some(path) = config.config_file.as_ref() {
		tracing::info!("using config file: {path}");
	}

	if let Some(limit) = config.memory.limit {
		tracing::info!("setting memory limit to {limit} bytes");
		ALLOCATOR.set_limit(limit).expect("failed to set memory limit");
	}

	tracing::info!("starting event-api");

	let (ctx, handler) = Context::new();

	let global = Arc::new(global::Global::new(ctx, config).await.expect("failed to initialize global"));

	let mut signal = scuffle_utils::signal::SignalHandler::new()
		.with_signal(SignalKind::interrupt())
		.with_signal(SignalKind::terminate());

	let health_handle = tokio::spawn(shared::health::run(global.clone()));
	let metrics_handle = tokio::spawn(shared::metrics::run(global.clone()));

	tokio::select! {
		_ = signal.recv() => tracing::info!("received shutdown signal"),
		r = health_handle => tracing::warn!("health server exited: {:?}", r),
		r = metrics_handle => tracing::warn!("metrics server exited: {:?}", r),
	}

	drop(global);

	tokio::select! {
		_ = signal.recv() => tracing::info!("received second shutdown signal, forcing exit"),
		r = handler.cancel().timeout(std::time::Duration::from_secs(60)) => {
			if r.is_err() {
				tracing::warn!("failed to cancel context in time, force exit");
			}
		}
	}

	tracing::info!("stopping event-api");
	std::process::exit(0);
}

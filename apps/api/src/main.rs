use global::Global;
use scuffle_bootstrap_telemetry::TelemetrySvc;
use scuffle_signal::SignalSvc;
mod cdn_purge;
mod config;
mod connections;
mod cron;
mod dataloader;
mod global;
mod http;
mod image_processor;
mod jwt;
mod mutex;
mod paypal_api;
mod ratelimit;
mod search;
mod stripe_client;
mod stripe_common;
mod sub_refresh_job;
mod transactions;

scuffle_bootstrap::main! {
	Global {
		http::run,
		image_processor::run,
		cron::run,
		cdn_purge::run,
		SignalSvc,
		TelemetrySvc,
	}
}

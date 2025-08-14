//! Run with `$ cargo run -p opentelemetry-exporter-geneva --example basic_msi`
//!
//! Required env for Geneva Config request (not auth-specific):
//!   GENEVA_ENDPOINT, GENEVA_ENVIRONMENT, GENEVA_ACCOUNT, GENEVA_NAMESPACE,
//!   GENEVA_REGION, GENEVA_CONFIG_MAJOR_VERSION
//!
//! Managed Identity selection (one of):
//!   GENEVA_MSI_CLIENT_ID or GENEVA_MSI_RESOURCE_ID
//!
//! Audience override (optional; defaults to GENEVA_ENDPOINT origin):
//!   GENEVA_AAD_SCOPE or GENEVA_AAD_RESOURCE

use geneva_uploader::client::{GenevaClient, GenevaClientConfig};
use geneva_uploader::AuthMethod;
use opentelemetry_appender_tracing::layer;
use opentelemetry_exporter_geneva::GenevaExporter;
use opentelemetry_sdk::logs::log_processor_with_async_runtime::BatchLogProcessor;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::{
    logs::{BatchConfig, SdkLoggerProvider},
    Resource,
};
use std::env;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    // Geneva Config inputs
    let endpoint = env::var("GENEVA_ENDPOINT").expect("GENEVA_ENDPOINT is required");
    let environment = env::var("GENEVA_ENVIRONMENT").expect("GENEVA_ENVIRONMENT is required");
    let account = env::var("GENEVA_ACCOUNT").expect("GENEVA_ACCOUNT is required");
    let namespace = env::var("GENEVA_NAMESPACE").expect("GENEVA_NAMESPACE is required");
    let region = env::var("GENEVA_REGION").expect("GENEVA_REGION is required");
    let config_major_version: u32 = env::var("GENEVA_CONFIG_MAJOR_VERSION")
        .expect("GENEVA_CONFIG_MAJOR_VERSION is required")
        .parse()
        .expect("GENEVA_CONFIG_MAJOR_VERSION must be a u32");

    // Identity context for metadata in uploads (not related to MSI auth)
    let tenant = env::var("GENEVA_TENANT").unwrap_or_else(|_| "default-tenant".to_string());
    let role_name = env::var("GENEVA_ROLE_NAME").unwrap_or_else(|_| "default-role".to_string());
    let role_instance =
        env::var("GENEVA_ROLE_INSTANCE").unwrap_or_else(|_| "default-instance".to_string());

    // Auth: Managed Identity
    // Note: selection and audience are read by the client via env:
    //   GENEVA_MSI_CLIENT_ID or GENEVA_MSI_RESOURCE_ID
    //   GENEVA_AAD_SCOPE or GENEVA_AAD_RESOURCE (optional)
    let config = GenevaClientConfig {
        endpoint,
        environment,
        account,
        namespace,
        region,
        config_major_version,
        auth_method: AuthMethod::ManagedIdentity,
        tenant,
        role_name,
        role_instance,
    };

    let geneva_client = GenevaClient::new(config)
        .await
        .expect("Failed to create GenevaClient with MSI");

    let exporter = GenevaExporter::new(geneva_client);
    let batch_processor = BatchLogProcessor::builder(exporter, Tokio)
        .with_batch_config(BatchConfig::default())
        .build();

    let provider: SdkLoggerProvider = SdkLoggerProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name("geneva-exporter-example-msi")
                .build(),
        )
        .with_log_processor(batch_processor)
        .build();

    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("opentelemetry=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    let otel_layer = layer::OpenTelemetryTracingBridge::new(&provider).with_filter(filter_otel);

    let filter_fmt = EnvFilter::new("info")
        .add_directive("hyper=debug".parse().unwrap())
        .add_directive("reqwest=debug".parse().unwrap())
        .add_directive("opentelemetry=debug".parse().unwrap());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_filter(filter_fmt);

    tracing_subscriber::registry()
        .with(otel_layer)
        .with(fmt_layer)
        .init();

    // Sample events
    info!(name: "Log", target: "my-system", event_id = 20, message = "Registration successful");
    info!(name: "Log", target: "my-system", event_id = 51, message = "Checkout successful");
    info!(name: "Log", target: "my-system", event_id = 30, message = "User login successful");
    info!(name: "Log", target: "my-system", event_id = 54, message = "Order shipped successfully");
    error!(name: "Log", target: "my-system", event_id = 31, message = "Login failed - invalid credentials");
    warn!(name: "Log", target: "my-system", event_id = 53, message = "Shopping cart abandoned");

    println!("Sleeping for 5 seconds...");
    thread::sleep(Duration::from_secs(5));
    let _ = provider.shutdown();
    println!("Shutting down provider");
}

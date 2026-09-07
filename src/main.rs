mod auth;
mod config;
mod image_ops;
mod proxy;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use config::{AppState, CliArgs};
use proxy::{
    get_hybrid, get_icon, get_overlay, get_road, get_road_dark, get_satellite, get_shield,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "apple_map_proxy=info,tower_http=info".into()),
        )
        .init();

    let args = CliArgs::parse();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .timeout(Duration::from_secs(15))
        .pool_max_idle_per_host(50)
        .tcp_keepalive(Duration::from_secs(60))
        .build()?;

    let state = Arc::new(AppState::new(&args, client));

    let cors = CorsLayer::permissive();

    let app = Router::new()
        // Road
        .route("/road/{z}/{x}/{y}", get(get_road))
        .route("/standard/{z}/{x}/{y}", get(get_road))
        .route("/road-dark/{z}/{x}/{y}", get(get_road_dark))
        .route("/dark/{z}/{x}/{y}", get(get_road_dark))
        // Satellite
        .route("/hybrid/{z}/{x}/{y}", get(get_hybrid))
        .route("/overlay/{z}/{x}/{y}", get(get_overlay))
        .route("/satellite/{z}/{x}/{y}", get(get_satellite))
        .route("/{z}/{x}/{y}", get(get_satellite))
        // Metadata
        .route("/md/v1/icon", get(get_icon))
        .route("/md/v1/shield", get(get_shield))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    info!("Starting Apple Maps tile proxy on http://{} (Offline Crypto Authentication)", bind_addr);
    println!("Starting Apple Maps tile proxy on http://{} (Offline Crypto Authentication)", bind_addr);
    println!("  Road (Light):    http://{}/road/{{z}}/{{x}}/{{y}}", bind_addr);
    println!("  Road (Dark):     http://{}/dark/{{z}}/{{x}}/{{y}} or /road-dark/{{z}}/{{x}}/{{y}}", bind_addr);
    println!("  Satellite tiles: http://{}/satellite/{{z}}/{{x}}/{{y}} (or /{{z}}/{{x}}/{{y}})", bind_addr);
    println!("  Hybrid tiles:    http://{}/hybrid/{{z}}/{{x}}/{{y}}", bind_addr);
    println!("  Overlay tiles:   http://{}/overlay/{{z}}/{{x}}/{{y}}", bind_addr);
    println!("  Icons:           http://{}/md/v1/icon?... ", bind_addr);
    println!("  Shields:         http://{}/md/v1/shield?... ", bind_addr);
    if args.cache_size_mb > 0 {
        println!("  Cache:           Enabled ({} MB, TTL {}s)", args.cache_size_mb, args.cache_ttl_secs);
    } else {
        println!("  Cache:           Disabled");
    }

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("\nShutdown signal received, shutting down gracefully...");
}

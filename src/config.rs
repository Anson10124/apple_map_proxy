use std::sync::Arc;
use clap::Parser;
use crate::auth::{AppleAuthenticator, DEFAULT_SECRET_PREFIX};

pub const DEFAULT_SATELLITE_VERSION: &str = "10421";
pub const DEFAULT_OVERLAY_VERSION: &str = "2609034";
pub const DEFAULT_ROAD_VERSION: &str = "2609034";

#[derive(Parser, Debug, Clone)]
#[command(
    name = "apple-map-proxy",
    about = "High-performance Apple Maps tile proxy with offline cryptographic URL signing",
    version
)]
pub struct CliArgs {
    #[arg(short = 'H', long, default_value = "0.0.0.0", env = "HOST")]
    pub host: String,

    #[arg(short, long, default_value_t = 8080, env = "PORT")]
    pub port: u16,

    #[arg(long, default_value = DEFAULT_SECRET_PREFIX, env = "SECRET_PREFIX")]
    pub secret_prefix: String,

    #[arg(long, default_value = DEFAULT_SATELLITE_VERSION, env = "SATELLITE_VERSION")]
    pub satellite_version: String,

    #[arg(long, default_value = DEFAULT_OVERLAY_VERSION, env = "OVERLAY_VERSION")]
    pub overlay_version: String,

    #[arg(long, default_value = DEFAULT_ROAD_VERSION, env = "ROAD_VERSION")]
    pub road_version: String,

    #[arg(long, default_value_t = 256, env = "CACHE_SIZE_MB")]
    pub cache_size_mb: u64,

    #[arg(long, default_value_t = 86400, env = "CACHE_TTL_SECS")]
    pub cache_ttl_secs: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub client: reqwest::Client,
    pub auth: Arc<AppleAuthenticator>,
    pub satellite_version: String,
    pub overlay_version: String,
    pub road_version: String,
    pub cache: Option<moka::future::Cache<String, (String, bytes::Bytes)>>,
}

impl AppState {
    pub fn new(args: &CliArgs, client: reqwest::Client) -> Self {
        let auth = AppleAuthenticator::new(&args.secret_prefix, None);
        let cache = if args.cache_size_mb > 0 {
            let max_capacity_bytes = args.cache_size_mb * 1024 * 1024;
            Some(
                moka::future::Cache::builder()
                    .weigher(|_key: &String, (_media_type, data): &(String, bytes::Bytes)| -> u32 {
                        data.len().min(u32::MAX as usize) as u32
                    })
                    .max_capacity(max_capacity_bytes)
                    .time_to_live(std::time::Duration::from_secs(args.cache_ttl_secs))
                    .build(),
            )
        } else {
            None
        };

        Self {
            client,
            auth: Arc::new(auth),
            satellite_version: args.satellite_version.clone(),
            overlay_version: args.overlay_version.clone(),
            road_version: args.road_version.clone(),
            cache,
        }
    }
}

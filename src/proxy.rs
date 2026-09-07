use std::collections::HashMap;
use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Deserializer};
use url::form_urlencoded;

use crate::config::AppState;
use crate::image_ops::composite_hybrid_tile;

const CACHE_CONTROL_HEADER: &str = "public, max-age=86400";
const CORS_HEADER: &str = "*";
const REFERER_HEADER: &str = "https://maps.apple.com/";

fn default_true() -> bool {
    true
}

fn default_light() -> String {
    "light".to_string()
}

fn default_scale() -> u32 {
    1
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_emphasis() -> String {
    "standard".to_string()
}

fn deserialize_bool_lenient<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrString {
        Bool(bool),
        Int(i64),
        Str(String),
    }

    match BoolOrString::deserialize(deserializer)? {
        BoolOrString::Bool(b) => Ok(b),
        BoolOrString::Int(i) => Ok(i != 0),
        BoolOrString::Str(s) => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            _ => Err(serde::de::Error::custom("expected boolean")),
        },
    }
}

#[derive(Debug, Deserialize)]
pub struct RoadQueryParams {
    #[serde(default = "default_light")]
    pub tint: String,
    #[serde(default = "default_scale")]
    pub scale: u32,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_true", deserialize_with = "deserialize_bool_lenient")]
    pub poi: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_bool_lenient")]
    pub labels: bool,
    #[serde(default = "default_emphasis")]
    pub emphasis: String,
}

#[derive(Debug, Deserialize)]
pub struct DarkQueryParams {
    #[serde(default = "default_scale")]
    pub scale: u32,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_true", deserialize_with = "deserialize_bool_lenient")]
    pub poi: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_bool_lenient")]
    pub labels: bool,
    #[serde(default = "default_emphasis")]
    pub emphasis: String,
}

#[inline]
pub fn clean_coord(y: &str) -> &str {
    y.split('.').next().unwrap_or(y)
}

fn build_response(status: StatusCode, media_type: &str, body: Bytes) -> Response {
    let mut resp = (status, body).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(media_type).unwrap_or(HeaderValue::from_static("image/png")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL_HEADER),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static(CORS_HEADER),
    );
    resp
}

async fn fetch_upstream(
    state: &AppState,
    raw_url: &str,
    referer: Option<&str>,
) -> Result<reqwest::Response, (StatusCode, String)> {
    let signed_url = state
        .auth
        .authenticate_url(raw_url, 4200)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Signing error: {e}")))?;

    let mut req = state.client.get(&signed_url);
    if let Some(ref_val) = referer {
        req = req.header(header::REFERER, ref_val);
    }

    req.send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Upstream request failed: {e}")))
}

async fn fetch_upstream_tile(
    state: &AppState,
    raw_url: &str,
    default_media_type: &str,
    referer: Option<&str>,
) -> Result<(String, Bytes), (StatusCode, String)> {
    let resp = fetch_upstream(state, raw_url, referer).await?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    if !status.is_success() {
        return Err((status, format!("Upstream request failed with status: {status}")));
    }

    let media_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or(default_media_type)
        .to_string();

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok((media_type, bytes))
}

async fn fetch_tile_or_cache(
    state: &AppState,
    cache_key: &str,
    raw_url: &str,
    default_media_type: &str,
    referer: Option<&str>,
) -> Result<(String, Bytes), (StatusCode, String)> {
    if let Some(cache) = &state.cache {
        let key = cache_key.to_string();
        let raw_url = raw_url.to_string();
        let default_media_type = default_media_type.to_string();
        let referer = referer.map(|s| s.to_string());
        let state_clone = state.clone();

        cache
            .try_get_with(key, async move {
                fetch_upstream_tile(
                    &state_clone,
                    &raw_url,
                    &default_media_type,
                    referer.as_deref(),
                )
                .await
            })
            .await
            .map_err(|e| (*e).clone())
    } else {
        fetch_upstream_tile(state, raw_url, default_media_type, referer).await
    }
}

pub async fn get_road(
    State(state): State<Arc<AppState>>,
    Path((z, x, y)): Path<(u32, u32, String)>,
    Query(params): Query<RoadQueryParams>,
) -> Result<Response, (StatusCode, String)> {
    let clean_y = clean_coord(&y);
    let poi_str = if params.poi { "1" } else { "0" };
    let labels_str = if params.labels { "1" } else { "0" };

    let raw_url = format!(
        "https://cdn.apple-mapkit.com/ti/tile?style=0&size=2&scale={}&x={}&y={}&z={}&v={}&lang={}&poi={}&labels={}&tint={}&emphasis={}",
        params.scale,
        x,
        clean_y,
        z,
        state.road_version,
        params.lang,
        poi_str,
        labels_str,
        params.tint,
        params.emphasis
    );

    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/png", Some(REFERER_HEADER)).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

pub async fn get_road_dark(
    State(state): State<Arc<AppState>>,
    Path((z, x, y)): Path<(u32, u32, String)>,
    Query(params): Query<DarkQueryParams>,
) -> Result<Response, (StatusCode, String)> {
    let clean_y = clean_coord(&y);
    let poi_str = if params.poi { "1" } else { "0" };
    let labels_str = if params.labels { "1" } else { "0" };

    let raw_url = format!(
        "https://cdn.apple-mapkit.com/ti/tile?style=0&size=2&scale={}&x={}&y={}&z={}&v={}&lang={}&poi={}&labels={}&tint=dark&emphasis={}",
        params.scale,
        x,
        clean_y,
        z,
        state.road_version,
        params.lang,
        poi_str,
        labels_str,
        params.emphasis
    );

    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/png", Some(REFERER_HEADER)).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

pub async fn get_satellite(
    State(state): State<Arc<AppState>>,
    Path((z, x, y)): Path<(u32, u32, String)>,
) -> Result<Response, (StatusCode, String)> {
    let clean_y = clean_coord(&y);
    let raw_url = format!(
        "https://sat-cdn.apple-mapkit.com/tile?style=7&size=2&scale=2&z={}&x={}&y={}&v={}",
        z, x, clean_y, state.satellite_version
    );

    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/jpeg", None).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

pub async fn get_overlay(
    State(state): State<Arc<AppState>>,
    Path((z, x, y)): Path<(u32, u32, String)>,
) -> Result<Response, (StatusCode, String)> {
    let clean_y = clean_coord(&y);
    let raw_url = format!(
        "https://cdn.apple-mapkit.com/ti/tile?style=46&size=2&scale=1&x={}&y={}&z={}&lang=en&v={}&poi=1",
        x, clean_y, z, state.overlay_version
    );

    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/png", Some(REFERER_HEADER)).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

async fn composite_hybrid_fetch(
    state: &AppState,
    z: u32,
    x: u32,
    clean_y: &str,
) -> Result<(String, Bytes), (StatusCode, String)> {
    let sat_url = format!(
        "https://sat-cdn.apple-mapkit.com/tile?style=7&size=2&scale=2&z={}&x={}&y={}&v={}",
        z, x, clean_y, state.satellite_version
    );
    let overlay_url = format!(
        "https://cdn.apple-mapkit.com/ti/tile?style=46&size=2&scale=1&x={}&y={}&z={}&lang=en&v={}&poi=1",
        x, clean_y, z, state.overlay_version
    );

    let (resp_sat, resp_overlay) = tokio::join!(
        fetch_tile_or_cache(state, &sat_url, &sat_url, "image/jpeg", None),
        fetch_tile_or_cache(state, &overlay_url, &overlay_url, "image/png", Some(REFERER_HEADER))
    );

    let (_, sat_bytes) = resp_sat?;

    if let Ok((_, overlay_bytes)) = resp_overlay {
        let sat_clone = sat_bytes.clone();
        let composite_result = tokio::task::spawn_blocking(move || {
            composite_hybrid_tile(&sat_clone, &overlay_bytes)
        })
        .await;

        if let Ok(Ok(composited_bytes)) = composite_result {
            return Ok(("image/jpeg".to_string(), composited_bytes));
        }
    }

    Ok(("image/jpeg".to_string(), sat_bytes))
}

pub async fn get_hybrid(
    State(state): State<Arc<AppState>>,
    Path((z, x, y)): Path<(u32, u32, String)>,
) -> Result<Response, (StatusCode, String)> {
    let clean_y = clean_coord(&y);
    let cache_key = format!(
        "hybrid:{}:{}:{}:v_sat={}:v_ov={}",
        z, x, clean_y, state.satellite_version, state.overlay_version
    );

    let (media_type, bytes) = if let Some(cache) = &state.cache {
        let key = cache_key;
        let state_clone = state.clone();
        let clean_y_str = clean_y.to_string();

        cache
            .try_get_with(key, async move {
                composite_hybrid_fetch(&state_clone, z, x, &clean_y_str).await
            })
            .await
            .map_err(|e| (*e).clone())?
    } else {
        composite_hybrid_fetch(&state, z, x, clean_y).await?
    };

    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

pub async fn get_icon(
    State(state): State<Arc<AppState>>,
    Query(mut params): Query<HashMap<String, String>>,
) -> Result<Response, (StatusCode, String)> {
    if !params.contains_key("v") {
        params.insert("v".to_string(), state.overlay_version.clone());
    }

    let query_string = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&params)
        .finish();

    let raw_url = format!("https://cdn.apple-mapkit.com/md/v1/icon?{query_string}");
    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/png", Some(REFERER_HEADER)).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

pub async fn get_shield(
    State(state): State<Arc<AppState>>,
    Query(mut params): Query<HashMap<String, String>>,
) -> Result<Response, (StatusCode, String)> {
    if !params.contains_key("v") {
        params.insert("v".to_string(), state.overlay_version.clone());
    }

    let query_string = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&params)
        .finish();

    let raw_url = format!("https://cdn.apple-mapkit.com/md/v1/shield?{query_string}");
    let (media_type, bytes) = fetch_tile_or_cache(&state, &raw_url, &raw_url, "image/png", Some(REFERER_HEADER)).await?;
    Ok(build_response(StatusCode::OK, &media_type, bytes))
}

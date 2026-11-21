//! HTTP layer: `/api/v1` routes, static web bundle, middleware.

pub mod error;

use std::time::Duration;

use axum::{
    Json, Router,
    http::{HeaderValue, Method, StatusCode, header},
    routing::get,
};
use serde::Serialize;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::config::Config;
use error::ApiError;

pub fn router(config: &Config) -> Router {
    let api_v1 = Router::new()
        .route("/health", get(health))
        .fallback(|| async { ApiError::not_found() });

    let mut app = Router::new().nest("/api/v1", api_v1);

    // Serve the exported Expo Web bundle (same origin as the API) when it exists.
    // Unknown paths fall back to index.html so expo-router deep links work.
    let index = config.web_dir.join("index.html");
    if index.is_file() {
        app = app.fallback_service(ServeDir::new(&config.web_dir).fallback(ServeFile::new(index)));
    } else {
        tracing::warn!(
            web_dir = %config.web_dir.display(),
            "web bundle not found; run `npm run export:web` in app/ to serve the app from this server"
        );
    }

    let mut app = app
        .layer(RequestBodyLimitLayer::new(16 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(TraceLayer::new_for_http());

    if let Some(cors) = cors_layer(&config.dev_cors_origins) {
        app = app.layer(cors);
    }
    app
}

fn cors_layer(origins: &[String]) -> Option<CorsLayer> {
    let origins: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    if origins.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]),
    )
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

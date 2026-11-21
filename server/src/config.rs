//! Runtime configuration, read from environment variables.

use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    /// Address the HTTP server binds to. `BIND_ADDR`, default `0.0.0.0:8080`.
    pub bind_addr: SocketAddr,
    /// Fixed LAN URL encoded in the QR code, e.g. `http://192.168.1.50:8080`. `PUBLIC_URL`.
    pub public_url: Option<String>,
    /// Directory holding migrations, card sets and the SQLite file. `DATA_DIR`, default `../data`.
    pub data_dir: PathBuf,
    /// Exported Expo Web bundle served at `/`. `WEB_DIR`, default `../app/dist`.
    pub web_dir: PathBuf,
    /// Origins allowed via CORS (dev only, e.g. Metro). `DEV_CORS_ORIGINS`, comma-separated.
    /// Debug builds default to `http://localhost:8081`; release builds default to none.
    pub dev_cors_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .context("BIND_ADDR must be host:port, e.g. 0.0.0.0:8080")?;

        let public_url = env::var("PUBLIC_URL").ok().filter(|s| !s.trim().is_empty());

        let dev_cors_origins = match env::var("DEV_CORS_ORIGINS") {
            Ok(v) => parse_list(&v),
            Err(_) if cfg!(debug_assertions) => vec!["http://localhost:8081".into()],
            Err(_) => Vec::new(),
        };

        Ok(Self {
            bind_addr,
            public_url,
            data_dir: env::var("DATA_DIR").unwrap_or_else(|_| "../data".into()).into(),
            web_dir: env::var("WEB_DIR").unwrap_or_else(|_| "../app/dist".into()).into(),
            dev_cors_origins,
        })
    }

    /// Config for tests: no CORS, nonexistent web dir.
    pub fn for_tests() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            public_url: None,
            data_dir: "../data".into(),
            web_dir: "./does-not-exist".into(),
            dev_cors_origins: Vec::new(),
        }
    }
}

fn parse_list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

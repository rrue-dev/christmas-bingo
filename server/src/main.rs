use bingo_server::{Config, router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    let app = router(&config);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("🎄 Christmas Bingo server listening on http://{}", listener.local_addr()?);
    if let Some(url) = &config.public_url {
        tracing::info!("Players join at {url}");
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await?;
    Ok(())
}

mod db;
mod model;
mod parser;
mod ranker;
mod service;
mod team;
mod web;
mod winrate;

use std::net::SocketAddr;

use anyhow::Context;
use db::Db;
use ranker::RankerConfig;
use service::AppService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("LANE_RANKER_DB")
        .unwrap_or_else(|_| "lane_ranker.sqlite3".to_string());
    let bind = std::env::var("LANE_RANKER_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let db = Db::open(&db_path)?;
    let config = RankerConfig::from_env();

    tracing::info!("lane ranker db: {db_path}");
    tracing::info!(
        warmup_rounds = config.warmup_rounds,
        total_rounds = config.total_rounds,
        win_rate_samples = config.win_rate_samples,
        stickiness = ?config.stickiness,
        outer_workers = config.outer_workers,
        inner_workers = config.inner_workers,
        skip_archived = config.skip_archived,
        "ranker config: outer_workers=0 means dynamic auto, outer_workers>0 means static chunks; inner_workers=0 means tswn-core auto; skip_archived=true skips archived combinations"
    );

    let app = web::router(AppService::new(db, config));

    let addr: SocketAddr = bind.parse().with_context(|| format!("invalid bind address: {bind}"))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

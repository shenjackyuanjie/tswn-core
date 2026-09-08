mod abcp_calibration;
mod db;
mod model;
mod name_profile;
mod parser;
mod ranker;
mod service;
mod web;
use anyhow::Context;
use std::net::SocketAddr;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let path = std::env::var("NAME_RANKER_DB").unwrap_or_else(|_| "name_ranker.sqlite3".into());
    let bind = std::env::var("NAME_RANKER_BIND").unwrap_or_else(|_| "127.0.0.1:3001".into());
    let service = service::Service::new(db::Db::open(&path)?)?;
    let addr: SocketAddr = bind.parse().with_context(|| format!("无效监听地址：{bind}"))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("name ranker: http://{addr}, db={path}");
    axum::serve(listener, web::router(service)).await?;
    Ok(())
}

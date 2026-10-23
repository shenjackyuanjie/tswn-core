//! 单号排名服务：独立 SQLite、ABCP 校准资源和 Axum HTTP 接口。
//!
//! 本 crate 在根 workspace 的 `exclude` 列表中，需要单独用 `--manifest-path`
//! 构建、验证或生成文档。
//! 编译期读取 `abcp_directional_calibrator.json` 与 `abcp_calibration_features.tsv`，
//! 构建前准备这两份独立校准资源；缺失时连单元测试也不能编译，不生成占位模型绕过检查。
//! ABCP 外部预测另外需要可执行文件、ONNX 与 scale，资源完整性要在部署环境单独验证。
//!
//! `web → service → ranker / name_profile → tswn_core`，`db` 持久化任务与排名。
//! `NAME_RANKER_DB` / `NAME_RANKER_BIND` 覆盖库路径和地址，默认本机端口 3001。
//! 启动和后台任务将结果写入数据库；它作为内部服务单独构建和部署。

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

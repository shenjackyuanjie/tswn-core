//! lane 排名与管理服务：SQLite 存储、core 胜率和 Axum Web 接口。
//!
//! # 运行与副作用
//!
//! `cargo run -p tswn_lane_ranker` 启动长期服务，默认监听 `127.0.0.1:3000`，
//! 初始化当前目录 `lane_ranker.sqlite3`。`LANE_RANKER_BIND` / `LANE_RANKER_DB`
//! 可覆盖地址和库路径；排名、归档、校准与靶子任务将结果保存到数据库。
//!
//! # 数据流
//!
//! `web → service 后台任务 → ranker / pairwise → winrate → tswn_core`，
//! `db` 保存组合、两两胜率、排名与审计。Raw/Golden 迭代在战斗结果之上表达排名策略。
//! 内层胜率 worker 固定为 1，外层按任务并行，集中管理计算并行度。
//! 常规 Correct 校准在 Rust 中使用固定槽位替换公式；严格验证和靶子压缩另行调用
//! Python 数值工具。Python 启动失败或返回值未通过校验时直接报告错误，不用占位结果替代。
//!
//! 这是按需构建和部署的应用 binary，通过 Web 接口使用。
//! 环境变量、校准依赖和模块说明见
//! [lane ranker README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_lane_ranker)。

/// SQLite 存储门面：连接、表访问与应用级任务状态。
mod db;
/// 领域模型：组合、lane 结果、进度与任务行类型。
mod model;
/// Correct 校准与严格 Python 验证入口。
mod pairwise;
/// 组合输入文本解析。
mod parser;
/// Raw/Golden 排名与自动封存编排。
mod ranker;
/// 应用服务与后台任务调度。
mod service;
/// 技能等效值与名字特征计算。
mod skill_eq;
/// 队伍合并查找（DSU）结构。
mod team;
/// Axum HTTP 接口与路由。
mod web;
/// core 胜率批量计算封装。
mod winrate;

use std::net::SocketAddr;

use anyhow::Context;
use db::Db;
use ranker::RankerConfig;
use service::AppService;

#[tokio::main]
/// 服务入口：初始化日志、按环境变量打开数据库与绑定地址，组装路由并启动 Axum。
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("LANE_RANKER_DB").unwrap_or_else(|_| "lane_ranker.sqlite3".to_string());
    let bind = std::env::var("LANE_RANKER_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

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
        "ranker config: outer_workers=0 means dynamic auto, outer_workers>0 means static chunks; inner_workers is fixed to 1; skip_archived=true skips archived combinations"
    );

    let app = web::router(AppService::new(db, config));

    let addr: SocketAddr = bind.parse().with_context(|| format!("invalid bind address: {bind}"))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

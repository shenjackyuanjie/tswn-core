//! `openbox-cli` 的计划类型：参数层与执行层之间的小型数据结构。
//!
//! 刻意保持为纯数据 + 少量纯函数，方便在单测里覆盖参数解析的边界，
//! 不触碰文件系统与后端。

use std::path::PathBuf;

use tswn_openbox::backend::NamerPfMetric;

/// `namer-pf --metric` 解析后的单项配置。
///
/// 语法 `NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]`，与 `tswn-cli namer-pf --metric`
/// 保持一致，两条 CLI 的输出行为因此相同。
#[derive(Debug, Clone)]
pub(super) struct MetricSpec {
    pub metric: NamerPfMetric,
    pub min_screen: Option<f64>,
    pub output_file: Option<PathBuf>,
    pub min_file: Option<f64>,
}

impl MetricSpec {
    pub(super) fn metric_label(&self) -> &'static str {
        match self.metric {
            NamerPfMetric::Pp => "pp",
            NamerPfMetric::Pd => "pd",
            NamerPfMetric::Qp => "qp",
            NamerPfMetric::Qd => "qd",
            NamerPfMetric::Sum => "sum",
        }
    }
}

/// 技能榜开关与落点。
///
/// `enabled = false` 时其余字段无意义；`config = None` 表示按 GUI 惯例读
/// `./setting/score_now.toml`，由后端解析。屏幕输出与否由 `enabled` 与
/// 顶层 `--no-screen` 在[tools::skill_board_options] 统一推导。
#[derive(Debug, Clone, Default)]
pub(super) struct SkillBoardPlan {
    pub enabled: bool,
    pub output_file: Option<PathBuf>,
    pub config: Option<PathBuf>,
}

use serde::{Deserialize, Serialize};

pub type GroupId = i64;
pub type JobId = i64;

#[derive(Debug, Clone, Serialize)]
pub struct SkillValue {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredGroup {
    pub id: GroupId,
    pub canonical: String,
    pub display_raw: String,
    pub lane_size: usize,
    pub team_name: String,
    pub members: Vec<String>,
    /// 手动屏蔽：仍参与 CQD/Score 等计算，但禁止被选入靶子。
    pub is_blocked: bool,
}

#[derive(Debug, Clone)]
pub struct RankNode {
    pub group: StoredGroup,
    pub cqd: f64,
    pub shenmixishu: f64,
    pub n: usize,
    pub bz: usize,
    pub odds_n: usize,
    pub cqdmin: f64,
    pub cqdmax: f64,
    pub cqds: f64,
    pub cqdss: f64,
}

impl RankNode {
    pub fn new(group: StoredGroup) -> Self {
        Self {
            group,
            cqd: 0.0,
            shenmixishu: 0.0,
            n: 0,
            bz: 0,
            odds_n: 0,
            cqdmin: f64::INFINITY,
            cqdmax: f64::NEG_INFINITY,
            cqds: 0.0,
            cqdss: 0.0,
        }
    }

    pub fn avg_cqd(&self) -> f64 {
        if self.n == 0 { self.cqd } else { self.cqds / self.n as f64 }
    }

    pub fn variance_cqd(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            let avg = self.avg_cqd();
            self.cqdss / self.n as f64 - avg * avg
        }
    }

    pub fn golden_rate(&self) -> f64 {
        if self.odds_n == 0 { 0.0 } else { self.bz as f64 / self.odds_n as f64 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneResultRow {
    pub lane_size: usize,
    pub group_id: GroupId,
    pub rank: usize,
    pub canonical: String,
    pub team_name: String,
    pub root_team_name: String,
    /// 原始 CQD/Score：仍由原来的收敛 ranker 产出，作为 pairwise calibration 的先验和回看标尺。
    pub average_cqd: f64,
    /// 原始 CQD 的稳定副本。旧版数据库没有该列时会回退到 average_cqd。
    pub raw_average_cqd: f64,
    /// Bradley-Terry pairwise calibration 后的全局标尺分。
    pub pair_score: Option<f64>,
    /// pair_score 在训练池内的裸排序。
    pub pair_rank: Option<usize>,
    /// pairwise 图拟合的不确定度；越小越稳。
    pub uncertainty: Option<f64>,
    /// pair_score - raw_average_cqd，用来观察旧 ranker 的高估/低估。
    pub raw_delta: Option<f64>,
    /// v49 多 seed edge-bagging 后 pair_score 的种子间标准差；越小越稳。
    pub pair_score_std: Option<f64>,
    /// v49 多 seed edge-bagging 后 pair_rank 的种子间标准差；越小越稳。
    pub pair_rank_std: Option<f64>,
    /// v49 多 seed edge-bagging 后 delta 的种子间标准差；越小越稳。
    pub delta_std: Option<f64>,
    /// v49 每个候选在 seed 图里的平均连接度。
    pub edge_count_mean: Option<f64>,
    /// stable / watch / unstable / below_threshold / blocked / calibration_skipped。
    pub stability_flag: String,
    /// 在不能重复号 + 战队上限下，强行让该组合进入合法榜后的总分边际变化。
    pub marginal_value: Option<f64>,
    /// 约束选择后的合法榜内名次；未入合法榜则为空。
    pub constrained_rank: Option<usize>,
    /// selected / candidate / below_threshold / blocked / calibration_skipped。
    pub selection_status: String,
    /// 用 tswn_lane_ranker/src/skill_eq.rs 内置 Rust 等效熟练度算法计算出的类型。
    pub type_label: String,
    /// 该组合所有成员、所有技能的等效熟练度合计；导出技能总表时使用。
    pub skill_totals: Vec<SkillValue>,
    pub min_cqd: f64,
    pub max_cqd: f64,
    pub variance_cqd: f64,
    pub golden_rate: f64,
    /// 手动屏蔽：仍参与 CQD/Score 等计算，但禁止被选入靶子。
    pub is_blocked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneProgress {
    pub lane_size: usize,
    pub phase: String,
    pub round: usize,
    pub total_rounds: usize,
    pub rate_done: usize,
    pub rate_total: usize,
    pub kicked_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneStatus {
    pub lane_size: usize,
    pub status: String,
    pub group_count: usize,
    pub progress: Option<LaneProgress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaneJob {
    pub id: JobId,
    pub lane_size: usize,
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
}


#[derive(Debug, Deserialize)]
pub struct BlockGroupRequest {
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。手动屏蔽组合本身不会被跳过；若同时已封存则仍按封存规则处理。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BlockGroupResponse {
    pub group_id: GroupId,
    pub lane_size: usize,
    pub canonical: String,
    pub blocked: bool,
    pub queued_lanes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct BlockGroupsByTextRequest {
    /// 每个元素是一个组合，例如 "aaa@A+bbb@A"。顺序会被 canonical 化。
    pub groups: Vec<String>,
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BlockGroupsByTextResponse {
    pub blocked: Vec<String>,
    pub unblocked: Vec<String>,
    pub ignored: Vec<IgnoredGroup>,
    pub queued_lanes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AddGroupsRequest {
    /// 每个元素是一个组合，例如 "aaa@A+bbb@A"。
    /// 前端也可以按行拆分后传进来。
    pub groups: Vec<String>,
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct IgnoredGroup {
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct AddGroupsResponse {
    pub added: Vec<String>,
    pub duplicated: Vec<String>,
    pub ignored: Vec<IgnoredGroup>,
    pub queued_lanes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MergeTeamsRequest {
    pub x: String,
    pub y: String,
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct MergeTeamsResponse {
    pub merged_root: String,
    pub queued_lanes: Vec<usize>,
}


#[derive(Debug, Deserialize)]
pub struct PurgeLowScoreGroupsRequest {
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PurgeLowScoreGroupsResponse {
    pub lane_size: usize,
    /// 严格小于该阈值才物理删除；等于阈值不删。
    pub threshold: f64,
    pub deleted_count: usize,
    pub queued_lanes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct RecomputeLaneRequest {
    /// 可选粘性。必须是正整数；不填则按 lane_size 使用默认值 10 * lane_size。
    pub stickiness: Option<usize>,
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 是否跳过已封存组合。默认 true。
    pub skip_archived: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RecomputeLaneResponse {
    pub queued_lanes: Vec<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ConstrainedSelectionRequest {
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 手动修正环境阈值。默认 48.5；不参与原靶子/默认排名流程。
    pub cqd_threshold: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ConstrainedSelectionResponse {
    pub queued_lanes: Vec<usize>,
    pub cqd_threshold: f64,
}

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
    pub average_cqd: f64,
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

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

    pub fn avg_cqd(&self) -> f64 { if self.n == 0 { self.cqd } else { self.cqds / self.n as f64 } }

    pub fn variance_cqd(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            let avg = self.avg_cqd();
            self.cqdss / self.n as f64 - avg * avg
        }
    }

    pub fn golden_rate(&self) -> f64 {
        if self.odds_n == 0 {
            0.0
        } else {
            self.bz as f64 / self.odds_n as f64
        }
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
    /// 校准后的全局标尺分；用于“展示校准/导出校准”。
    pub pair_score: Option<f64>,
    /// pair_score 在训练池内的裸排序。
    pub pair_rank: Option<usize>,
    /// pairwise 图拟合的不确定度；越小越稳。
    pub uncertainty: Option<f64>,
    /// pair_score - raw_average_cqd，用来观察旧 ranker 的高估/低估。
    pub raw_delta: Option<f64>,
    /// 在不能重复号 + 战队上限下，强行让该组合进入合法榜后的总分边际变化。
    pub marginal_value: Option<f64>,
    /// 约束选择后的合法榜内名次；未入合法榜则为空。
    pub constrained_rank: Option<usize>,
    /// selected / candidate / below_threshold / blocked / calibration_skipped。
    pub selection_status: String,
    /// 用 tswn_lane_ranker/src/skill_eq.rs 内置 Rust 等效熟练度算法计算出的原始文字/技能类型。
    /// 该字段保留旧口径：每个成员取等效熟练度 >= 25 的技能拼接，不能和胜率分布类型混用。
    pub type_label: String,
    /// 简化版文字主技能类型：每个号只取等效熟练度最高的主动技能，lane=2 时形如 `背刺+铁壁`。
    pub simple_type_label: String,
    /// 胜率分布聚类得到的类型，例如 `胜率型04 n=170`；不覆盖文字/技能类型。
    /// 当前口径是 direct win-rate shape：直接聚类候选对不同 Raw 段对手的胜率分布形状。
    pub winrate_type_label: Option<String>,
    /// direct win-rate shape profile 到所属中心的距离；越小越像该类型中心。
    pub winrate_profile_distance: Option<f64>,
    /// 到第二近类型中心的距离。和 distance 一起看可判断边界样本。
    pub winrate_profile_second_distance: Option<f64>,
    /// second_distance - distance；越大越确定，越小越像夹在两个类型之间。
    pub winrate_profile_margin: Option<f64>,
    /// soft membership 最大后验概率；越接近 1 越像单一类型，越低越像混合/边界样本。
    pub winrate_profile_soft_confidence: Option<f64>,
    /// soft membership 熵，按 cluster_count 归一化到 0..1；越高越混合。
    pub winrate_profile_soft_entropy: Option<f64>,
    /// 校准温度后的 soft membership 最大概率；用于 UI 判断“混合/边界型”。
    pub winrate_profile_soft_confidence_calibrated: Option<f64>,
    /// 校准温度后的 soft membership 归一化熵。
    pub winrate_profile_soft_entropy_calibrated: Option<f64>,
    /// 兼容字段：V10 起表示重新聚类 bootstrap 后保持同一等价胜率型的比例。
    pub winrate_profile_bootstrap_stability: Option<f64>,
    /// 固定主模型中心时的 assignment repeatability；不是聚类结构稳定性。
    pub winrate_profile_fixed_center_stability: Option<f64>,
    /// 重新生成 profile、重新 spectral embedding、重新聚类并对齐 ID 后的稳定性。
    pub winrate_profile_recluster_stability: Option<f64>,
    /// 重新聚类 bootstrap 后，原簇与对齐后簇的平均 Jaccard。
    pub winrate_profile_recluster_jaccard: Option<f64>,
    /// bootstrap 归属分布熵；越高表示经常落到不同等价簇。
    pub winrate_profile_assignment_entropy: Option<f64>,
    /// bootstrap 重聚类后的全局 Adjusted Rand Index 均值；每行重复导出，方便前端 summary。
    pub winrate_profile_recluster_ari: Option<f64>,
    /// spectral/net-win embedding 第一维，仅用于诊断和可视化，不参与分数。
    pub winrate_profile_embedding_x: Option<f64>,
    /// spectral/net-win embedding 第二维，仅用于诊断和可视化，不参与分数。
    pub winrate_profile_embedding_y: Option<f64>,
    /// 扣除 Raw score 差对应理论胜率后的 residual matchup 类型；用于诊断克制/机制，不参与分数。
    pub residual_type_label: Option<String>,
    /// residual matchup profile 到所属中心的距离。
    pub residual_profile_distance: Option<f64>,
    /// residual matchup profile 到第二近中心的距离。
    pub residual_profile_second_distance: Option<f64>,
    /// residual profile second_distance - distance。
    pub residual_profile_margin: Option<f64>,
    /// 校准后的 residual soft membership 最大概率。
    pub residual_profile_soft_confidence_calibrated: Option<f64>,
    /// 校准后的 residual soft membership 归一化熵。
    pub residual_profile_soft_entropy_calibrated: Option<f64>,
    /// residual profile 重新聚类 bootstrap 稳定性。
    pub residual_profile_recluster_stability: Option<f64>,
    /// residual profile bootstrap assignment 熵。
    pub residual_profile_assignment_entropy: Option<f64>,
    /// residual spectral/net-win embedding 第一维。
    pub residual_profile_embedding_x: Option<f64>,
    /// residual spectral/net-win embedding 第二维。
    pub residual_profile_embedding_y: Option<f64>,
    /// V16 residual shape 方差归一化前的加权 RMS；越小越像低方差/无刺候选。
    pub residual_profile_shape_rms: Option<f64>,
    /// V16 residual 方差特征，使用 ln(1 + variance / floor^2) 压缩后参与聚类。
    pub residual_profile_variance_feature: Option<f64>,
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
pub struct AddWinratesRequest {
    /// 每两行组成一组：第 1 行组合对战第 2 行组合，第 3 行组合对战第 4 行组合，以此类推。
    /// 只测已存在于数据库的组合；任一组合不存在则该 pair 不写入 group_rates。
    pub groups: Vec<String>,
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddedWinrateRow {
    pub pair_index: usize,
    pub group_a: String,
    pub group_b: String,
    pub group_a_id: GroupId,
    pub group_b_id: GroupId,
    pub lane_size: usize,
    pub win_rate_a: f64,
    pub win_rate_b: f64,
    pub samples: usize,
    pub stored: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IgnoredWinratePair {
    pub pair_index: usize,
    pub group_a: Option<String>,
    pub group_b: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct AddWinratesResponse {
    pub requested_pairs: usize,
    pub computed_pairs: usize,
    pub stored_pairs: usize,
    pub ignored_pairs: Vec<IgnoredWinratePair>,
    pub results: Vec<AddedWinrateRow>,
    pub samples: usize,
    pub outer_workers: usize,
    pub mode: String,
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

#[derive(Debug, Deserialize)]
pub struct ConstrainedSelectionRequest {
    /// 外层 worker。0 或不填 = 自动 worker + 动态队列；>0 = 指定 worker 数 + 静态分块。
    pub outer_workers: Option<usize>,
    /// 兼容旧请求字段；服务端固定忽略，实际 inner worker 永远为 1。
    pub inner_workers: Option<u32>,
    /// 校准池 Raw Score 门槛。前端“校准池 Raw Score ≥”输入直连到这里，
    /// 并透传给 strict Python calibrator 的 `--raw-min`。
    pub raw_score_threshold: Option<f64>,
    /// 兼容旧前端/旧 API 字段；如果 raw_score_threshold 为空才使用。
    pub cqd_threshold: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ConstrainedSelectionResponse {
    pub queued_lanes: Vec<usize>,
    pub raw_score_threshold: f64,
    /// 兼容旧前端字段；值与 raw_score_threshold 相同。
    pub cqd_threshold: f64,
}

#[derive(Debug, Deserialize)]
pub struct TargetGenerationRequest {
    /// 兼容旧版 50 靶 API。direct trace 模式不再按 C-Score 截断。
    pub cqd_threshold: Option<f64>,
    /// 兼容旧版 50 靶 API。direct trace 模式不再固定主榜前缀。
    pub fixed_main_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CorrectTargetTraceWeight {
    pub reference_scope: String,
    pub group_id: GroupId,
    /// 仅用于审计展示的归一化公共权重，所有行之和为 1。
    pub reference_weight: f64,
    /// 可直接导出的普通靶权重，等于 50 * common_coefficient。
    pub nominal_weight: f64,
    /// Raw 阶段记录的原始 Golden 权重。
    pub raw_golden_weight: f64,
    /// 从 Golden 出发、经稳定自由总质量残差路径选择的带符号公共 Correct 系数 a_g。
    pub common_coefficient: f64,
    /// 所有逐行 Correct 系数在该候选列上的无约束均值。
    pub coefficient_mean: f64,
    /// 所有逐行 Correct 系数在该候选列上的总体标准差。
    pub coefficient_stddev: f64,
    /// 可直接用于普通带权打靶的权重 50*a_g。
    pub correct_target_weight: f64,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct CorrectTargetTrace {
    pub trace_version: String,
    pub score_mode: String,
    pub metadata_json: String,
    pub weights: Vec<CorrectTargetTraceWeight>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetGenerationRow {
    pub target_rank: usize,
    /// 入选项的大靶原权重加上从未入选大靶行搬运来的尾部增量。
    pub target_weight: f64,
    /// 该入选项在完整 Correct 大靶中的原始权重。
    #[serde(rename = "big_target_base_weight")]
    pub base_weight: Option<f64>,
    /// 锁定锚点加大靶画像覆盖选号后、进入最终调平前的初始权重。
    #[serde(rename = "seed_initial_weight")]
    pub seed_initial_weight: Option<f64>,
    /// 从所有未入选大靶行承接的带符号权重。
    #[serde(rename = "transported_tail_addition")]
    pub fitted_tail_weight: Option<f64>,
    /// 保持完整大靶总质量的内部血缘权重；导出权重由它按 n/M 缩放。
    pub lineage_weight: Option<f64>,
    /// locked_top3_proportional_mass_profile_seed_then_flatten
    pub phase: String,
    pub trace_component: String,
    /// 当前为 all_correct_candidates。
    pub trace_scope: String,
    pub trace_source: String,
    pub reference_weight: Option<f64>,
    pub group_id: GroupId,
    pub canonical: String,
    pub team_name: String,
    pub root_team_name: String,
    pub correct_rank: Option<usize>,
    pub correct_score: f64,
    pub raw_rank: usize,
    pub raw_score: f64,
    pub delta_rank: Option<i64>,
    pub selection_status: String,
    pub type_label: String,
    pub simple_type_label: String,
    /// 所有 reference 主榜行打这个靶子的平均胜率；越高表示靶子越容易被 reference 打。
    pub average_reference_winrate: Option<f64>,
    pub reference_rate_count: usize,
    pub player_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetReferenceAuditRow {
    pub reference_rank: usize,
    pub group_id: GroupId,
    pub canonical: String,
    pub team_name: String,
    pub root_team_name: String,
    pub correct_rank: Option<usize>,
    pub correct_score: f64,
    pub raw_rank: usize,
    pub raw_score: f64,
    /// 该 reference 打 50 个靶子的真实平均胜率，未做平移/压缩。
    pub average_winrate_vs_targets: Option<f64>,
    pub target_rate_count: usize,
    /// 将 average_winrate_vs_targets 按 reference 均值/方差线性对齐到 C-Score 标尺后的值。
    pub aligned_c_score_from_targets: Option<f64>,
    /// aligned_c_score_from_targets - correct_score。
    pub aligned_minus_c_score: Option<f64>,
    pub abs_aligned_minus_c_score: Option<f64>,
    pub type_label: String,
    pub simple_type_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetGenerationSummary {
    pub algorithm: String,
    pub trace_version: String,
    pub score_mode: String,
    pub lane_size: usize,
    /// 压缩后实际保留的靶子数量 n。
    pub target_count: usize,
    /// 直接打靶公式 Σ(rate*weight)/n 中的分母 n。
    pub score_denominator: usize,
    pub unique_group_count: usize,
    pub raw_trace_count: usize,
    pub correct_trace_count: usize,
    pub correct_reference_scope_count: usize,
    pub raw_weight_sum: f64,
    pub correct_nominal_weight_sum: f64,
    pub correct_target_weight_sum: f64,
    pub correct_target_candidate_count: usize,
    pub correct_target_nonzero_count: usize,
    /// 公共 a_g 对已有 Correct Score 的平均绝对前向误差。
    pub correct_forward_replay_mean_abs_diff: f64,
    pub correct_forward_replay_max_abs_diff: f64,
    pub correct_forward_replay_rmse: f64,
    /// 逐行系数追溯对已有 Correct Score 的重放误差；应只含浮点噪声。
    pub row_coefficient_replay_mean_abs_diff: f64,
    pub row_coefficient_replay_max_abs_diff: f64,
    pub exact_replay_requires_trace_semantics: bool,
    /// 兼容旧 API 字段名；当前含义为压缩前完整 Correct 大靶权重和。
    pub merged_weight_sum_before_normalization: f64,
    pub merged_normalization_scale: f64,
    pub support_base_weight_sum: f64,
    pub tail_group_count: usize,
    pub tail_weight_sum: f64,
    pub fitted_addition_sum: f64,
    pub fit_baseline_mean_abs_diff: f64,
    pub fit_baseline_max_abs_diff: f64,
    pub fit_baseline_rmse: f64,
    pub fit_optimized_mean_abs_diff: f64,
    pub fit_optimized_max_abs_diff: f64,
    pub fit_optimized_rmse: f64,
    pub fit_weight_regularization_applied: bool,
    pub fit_baseline_addition_l2_norm: f64,
    pub fit_optimized_addition_l2_norm: f64,
    pub fit_optimized_addition_max_abs: f64,
    pub fit_optimized_distance_from_proportional_l2: f64,
    /// 以下四个字段保留旧 API 兼容。
    pub fit_final_weight_std_limit: f64,
    pub fit_final_weight_max_deviation_limit: f64,
    pub fit_optimized_final_weight_std: f64,
    pub fit_optimized_final_weight_max_deviation: f64,
    /// 按最终 /n 计分时，压缩靶相对既有 Correct Score 的压平误差。
    pub compressed_replay_mean_abs_diff: f64,
    pub compressed_replay_max_abs_diff: f64,
    pub compressed_replay_rmse: f64,
    /// 以下字段保留旧版 API 兼容；新流程不执行旧 fixed/MILP。
    pub fixed_main_count: usize,
    pub optimized_count: usize,
    pub player_cap: usize,
    pub target_weight_sum: f64,
    pub target_weight_min: f64,
    pub target_weight_max: f64,
    pub player_weight_cap: f64,
    pub cqd_threshold: f64,
    pub reference_limit: usize,
    pub reference_count: usize,
    pub candidate_count: usize,
    pub objective_mse: f64,
    pub objective_corr: Option<f64>,
    pub reference_avg_winrate_mean: f64,
    pub reference_avg_winrate_std: f64,
    pub reference_c_score_mean: f64,
    pub reference_c_score_std: f64,
    pub audit_reference_rows: usize,
    pub audit_mean_diff: Option<f64>,
    pub audit_mean_abs_diff: Option<f64>,
    pub audit_max_abs_diff: Option<f64>,
    pub audit_rmse: Option<f64>,
    pub audit_p95_abs_diff: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetGenerationResponse {
    pub summary: TargetGenerationSummary,
    /// 最终可直接使用的 `weight<TAB>canonical` 可变数量配置。
    pub target_config_text: String,
    /// Correct 重放公式及常量，由产生当前 Correct 分数的校准运行记录。
    pub trace_metadata: serde_json::Value,
    pub rows: Vec<TargetGenerationRow>,
    pub reference_audit_rows: Vec<TargetReferenceAuditRow>,
}

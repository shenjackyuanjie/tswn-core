#ifndef TSWN_CAPI_H
#define TSWN_CAPI_H

/*
 * tswn_capi.h
 *
 * `tswn_capi` 提供基于 `tswn_core` 的 DLL C-API。
 *
 * 基本约定：
 * - 输入字符串统一为 UTF-8 且以 `\0` 结尾的 `const char*`
 * - 库返回的动态字符串统一使用 `tswn_str_t`
 * - 库返回的动态字节统一使用 `tswn_bytes_t`
 * - `tswn_str_t` / `tswn_bytes_t` 必须分别通过 `tswn_str_free` / `tswn_bytes_free` 释放
 * - `win_rate` 相关接口只返回 `wins` / `total`，百分比由调用方自行计算
 * - 推荐新调用方优先使用下方“CLI 对齐高层接口”与 `tswn_battle_replay_json`
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum tswn_status_t {
    TSWN_OK = 0,
    TSWN_ERR_NULL = 1,
    TSWN_ERR_INVALID_UTF8 = 2,
    TSWN_ERR_INVALID_ARGUMENT = 3,
    TSWN_ERR_RUNNER = 4,
    TSWN_ERR_PANIC = 255
} tswn_status_t;

/* UTF-8 字符串切片。由库分配，调用方负责释放。 */
typedef struct tswn_str_t {
    const char* ptr;
    size_t len;
} tswn_str_t;

/* 原始字节切片。由库分配，调用方负责释放。 */
typedef struct tswn_bytes_t {
    const uint8_t* ptr;
    size_t len;
} tswn_bytes_t;

/* 胜率统计结果。 */
typedef struct tswn_win_rate_result_t {
    uint64_t wins;
    uint64_t total;
} tswn_win_rate_result_t;

/* 玩家只读快照。 */
typedef struct tswn_player_snapshot_t {
    uint64_t id;
    uint64_t ptr;
    int32_t hp;
    int32_t max_hp;
    int32_t magic_point;
    int32_t move_point;
    int32_t attack;
    int32_t defense;
    int32_t speed;
    int32_t agility;
    int32_t magic;
    int32_t resistance;
    int32_t wisdom;
    uint32_t point;
    uint32_t all_sum;
    double name_factor;
    double at_boost;
    double attract;
    uint8_t frozen;
} tswn_player_snapshot_t;

typedef enum tswn_update_type_t {
    TSWN_UPDATE_WIN = 0,
    TSWN_UPDATE_NONE = 1,
    TSWN_UPDATE_NEXT_LINE = 2
} tswn_update_type_t;

/* 单条更新帧的轻量快照。 */
typedef struct tswn_update_snapshot_t {
    uint32_t score;
    uint32_t param;
    int32_t delay0;
    int32_t delay1;
    uint64_t caster_id;
    uint64_t target_id;
    size_t target_count;
    uint8_t has_param;
    tswn_update_type_t update_type;
} tswn_update_snapshot_t;

/* 规范用户 API。通过追加导出保持 ABI 4 兼容。 */
typedef struct tswn_battle_session_t tswn_battle_session_t;
typedef struct tswn_battle_options_t {
    uint32_t struct_size;
    double eval_rq;
    size_t max_rounds;
    uint8_t include_icons;
} tswn_battle_options_t;
typedef enum tswn_battle_status_t {
    TSWN_BATTLE_RUNNING = 0, TSWN_BATTLE_FINISHED = 1, TSWN_BATTLE_TRUNCATED = 2
} tswn_battle_status_t;
typedef enum tswn_battle_stop_reason_t {
    TSWN_BATTLE_STOP_NONE = 0, TSWN_BATTLE_STOP_WINNER = 1,
    TSWN_BATTLE_STOP_MAX_ROUNDS = 2, TSWN_BATTLE_STOP_NO_PROGRESS = 3
} tswn_battle_stop_reason_t;

/* 覆写字段前先初始化。default 永久仅写 V1，并设置 struct_size=V1_SIZE；
 * 未来扩展须新增带容量参数的初始化接口，禁止扩大此函数的写入范围。
 * new 接受 NULL options 以使用默认值。
 * struct_size 小于 V1 最小结构尺寸会被拒绝；历史 V1 prefix 永久有效。
 * 旧 caller 可调用新 library：未提供的新尾字段使用默认值；未知尾部由旧 library 忽略。
 * 未来字段 offset 必须 >= V1_SIZE，禁止复用 V1 tail padding，必要时增加显式 padding。
 * 每个存活句柄必须释放一次，且不得并发使用。
 * 所有返回字符串归调用方所有；使用 tswn_str_free 释放。
 * 可选输出在缺失时使用 has=0 和 {NULL,0}，包括重复的终止调用。
 */
void tswn_battle_options_default(tswn_battle_options_t* options);
tswn_status_t tswn_battle_session_new(const char* raw_text_utf8, const tswn_battle_options_t* options, tswn_battle_session_t** out_session);
void tswn_battle_session_free(tswn_battle_session_t* session);
/* Runtime error 后为 1，result/stop_reason 不存在且后续推进返回相同 sticky error。
 * 失败不属于 truncated，调用方应停止推进并释放 session。NULL 返回 0，不修改 last_error。 */
uint8_t tswn_battle_session_is_failed(const tswn_battle_session_t* session);
tswn_status_t tswn_battle_session_initial_states_json(const tswn_battle_session_t* session, tswn_str_t* out_json);
tswn_status_t tswn_battle_session_current_states_json(const tswn_battle_session_t* session, tswn_str_t* out_json);
tswn_status_t tswn_battle_session_next_frame_json(tswn_battle_session_t* session, uint8_t* out_has_frame, tswn_str_t* out_json);
tswn_status_t tswn_battle_session_result_json(const tswn_battle_session_t* session, uint8_t* out_has_result, tswn_str_t* out_json);
tswn_status_t tswn_battle_session_status(const tswn_battle_session_t* session, tswn_battle_status_t* out_value);
tswn_status_t tswn_battle_session_stop_reason(const tswn_battle_session_t* session, tswn_battle_stop_reason_t* out_value);

typedef struct tswn_runner_t tswn_runner_t;
typedef struct tswn_prepared_runner_t tswn_prepared_runner_t;
typedef struct tswn_updates_t tswn_updates_t;

/* ABI / 版本 / 错误 */

/* 返回当前 C-API ABI 版本号。 */
uint32_t tswn_capi_abi_version(void);

/* 返回 `tswn_capi` 包装层版本字符串。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_capi_version(void);

/* 返回 `tswn_core` 版本字符串。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_core_version(void);

/* 返回普通对局默认使用的 eval_rq。 */
double tswn_default_eval_rq(void);

/* 返回 win-rate 语义默认使用的 eval_rq。 */
double tswn_win_rate_eval_rq(void);

/* 返回当前线程上最近一次错误消息。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_last_error_message(void);

/* 返回当前线程上最近一次错误的稳定错误码。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_last_error_code(void);

/* 清除当前线程上的最近一次错误消息。 */
void tswn_clear_error(void);

/* 释放由库返回的字符串。 */
void tswn_str_free(tswn_str_t value);

/* 释放由库返回的字节缓冲。 */
void tswn_bytes_free(tswn_bytes_t value);

/* 释放 Runner 句柄。 */
void tswn_runner_free(tswn_runner_t* ptr);

/* 释放 PreparedRunner 句柄。 */
void tswn_prepared_runner_free(tswn_prepared_runner_t* ptr);

/* 释放 RunUpdates 句柄。 */
void tswn_updates_free(tswn_updates_t* ptr);

/* Runner / PreparedRunner 生命周期 */

/* 从名竞原始输入构造 Runner，使用普通对局默认 eval_rq。 */
tswn_status_t tswn_runner_new_from_raw(const char* raw_text_utf8, tswn_runner_t** out_runner);

/* 从名竞原始输入构造 Runner，并显式指定 eval_rq。 */
tswn_status_t tswn_runner_new_from_raw_with_eval_rq(const char* raw_text_utf8, double eval_rq, tswn_runner_t** out_runner);

/* 从名竞原始输入构造 PreparedRunner，使用普通对局默认 eval_rq。 */
tswn_status_t tswn_prepared_runner_new_from_raw(const char* raw_text_utf8, tswn_prepared_runner_t** out_prepared);

/* 从名竞原始输入构造 PreparedRunner，并显式指定 eval_rq。 */
tswn_status_t tswn_prepared_runner_new_from_raw_with_eval_rq(const char* raw_text_utf8, double eval_rq, tswn_prepared_runner_t** out_prepared);

/* 通过 PreparedRunner 和可选 seed 构造 Runner。传入 NULL 表示不带 seed。 */
tswn_status_t tswn_runner_new_from_prepared(const tswn_prepared_runner_t* prepared, const char* seed_utf8, tswn_runner_t** out_runner);

/* Runner 执行 / 状态 */

/* 返回当前是否已经分出胜负。 */
uint8_t tswn_runner_have_winner(const tswn_runner_t* runner);

/* 将对局推进到结束，返回是否成功分出胜者。 */
uint8_t tswn_runner_run_to_completion(tswn_runner_t* runner);

/* 推进一个主回合，并返回本回合产生的更新帧句柄。 */
tswn_status_t tswn_runner_main_round(tswn_runner_t* runner, tswn_updates_t** out_updates);

/* 返回原始输入顺序对应的队伍数量。 */
size_t tswn_runner_input_group_count(const tswn_runner_t* runner);

/* 返回指定输入队伍的成员数量。 */
size_t tswn_runner_input_group_len(const tswn_runner_t* runner, size_t group_index);

/* 复制指定输入队伍的 roster 到调用方缓冲区。 */
tswn_status_t tswn_runner_input_group_copy(const tswn_runner_t* runner, size_t group_index, uint64_t* out_ids, size_t cap);

/* 查询指定玩家在原始输入中的队伍下标。 */
tswn_status_t tswn_runner_player_input_group_index(const tswn_runner_t* runner, uint64_t player_id, size_t* out_group_index);

/* 返回胜者 roster 长度；若尚未分出胜负则为 0。 */
size_t tswn_runner_winner_len(const tswn_runner_t* runner);

/* 复制胜者 roster 到调用方缓冲区。 */
tswn_status_t tswn_runner_winner_copy(const tswn_runner_t* runner, uint64_t* out_ids, size_t cap);

/* 返回当前对局中全部玩家数量。 */
size_t tswn_runner_all_player_count(const tswn_runner_t* runner);

/* 复制全部玩家 ID 到调用方缓冲区。 */
tswn_status_t tswn_runner_all_player_ids_copy(const tswn_runner_t* runner, uint64_t* out_ids, size_t cap);

/* 获取指定玩家的只读快照。 */
tswn_status_t tswn_runner_player_snapshot(const tswn_runner_t* runner, uint64_t player_id, tswn_player_snapshot_t* out_snapshot);

/* RunUpdates 访问 */

/* 返回更新帧数量。 */
size_t tswn_updates_len(const tswn_updates_t* updates);

/* 读取指定下标的更新帧快照。 */
tswn_status_t tswn_updates_get(const tswn_updates_t* updates, size_t index, tswn_update_snapshot_t* out_update);

/* 复制指定更新帧的 targets 列表。 */
tswn_status_t tswn_updates_targets_copy(const tswn_updates_t* updates, size_t index, uint64_t* out_ids, size_t cap);

/* 返回更新帧的原始消息模板。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_updates_message(const tswn_updates_t* updates, size_t index);

/* 返回更新帧的占位符展开结果。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_updates_message_rendered(const tswn_updates_t* updates, size_t index);

/* 高层胜率辅助接口 */

/*
 * 按默认 win-rate 语义计算第一组对其余组的胜率统计。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_win_rate(const char* raw_text_utf8, size_t n, uint32_t thread, tswn_win_rate_result_t* out_result);

/*
 * 按显式 eval_rq 计算第一组对其余组的胜率统计。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_win_rate_with_eval_rq(const char* raw_text_utf8, size_t n, uint32_t thread, double eval_rq, tswn_win_rate_result_t* out_result);

/*
 * 按默认 win-rate 语义批量计算 target 对多个 against 的胜率统计。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_group_win_rate(
    const char* target_utf8,
    const char* const* against_utf8,
    size_t against_len,
    size_t n,
    uint32_t thread,
    tswn_win_rate_result_t* out_results
);

/*
 * 按显式 eval_rq 批量计算 target 对多个 against 的胜率统计。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_group_win_rate_with_eval_rq(
    const char* target_utf8,
    const char* const* against_utf8,
    size_t against_len,
    size_t n,
    uint32_t thread,
    double eval_rq,
    tswn_win_rate_result_t* out_results
);

/*
 * 基于 PreparedRunner 按默认 win-rate 语义计算第一组对其余组的胜率统计。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_prepared_win_rate(const tswn_prepared_runner_t* prepared, size_t n, uint32_t thread, tswn_win_rate_result_t* out_result);

/*
 * 基于 PreparedRunner 计算第一组对其余组的胜率统计，并验证 eval_rq 与创建模板时一致。
 * `thread` 语义：0=自动线程数，1=单线程，n=指定线程数。
 */
tswn_status_t tswn_prepared_win_rate_with_eval_rq(
    const tswn_prepared_runner_t* prepared,
    size_t n,
    uint32_t thread,
    double eval_rq,
    tswn_win_rate_result_t* out_result
);

/*
 * CLI 对齐的高层辅助接口。
 * 复杂返回值统一编码为 JSON UTF-8 字符串，并由调用方使用 `tswn_str_free` 释放。
 */
tswn_status_t tswn_win_rate_summary_json(const char* raw_text_utf8, size_t n, uint32_t thread, tswn_str_t* out_json);
tswn_status_t tswn_win_rate_summary_json_with_eval_rq(
    const char* raw_text_utf8,
    size_t n,
    uint32_t thread,
    double eval_rq,
    tswn_str_t* out_json
);

tswn_status_t tswn_team_win_rate_summary_json(
    const char* team1_utf8,
    const char* team2_utf8,
    size_t n,
    uint32_t thread,
    tswn_str_t* out_json
);
tswn_status_t tswn_team_win_rate_summary_json_with_eval_rq(
    const char* team1_utf8,
    const char* team2_utf8,
    size_t n,
    uint32_t thread,
    double eval_rq,
    tswn_str_t* out_json
);

tswn_status_t tswn_group_win_rate_summary_json(
    const char* target_utf8,
    const char* const* against_utf8,
    size_t against_len,
    size_t n,
    uint32_t thread,
    tswn_str_t* out_json
);
tswn_status_t tswn_group_win_rate_summary_json_with_eval_rq(
    const char* target_utf8,
    const char* const* against_utf8,
    size_t against_len,
    size_t n,
    uint32_t thread,
    double eval_rq,
    tswn_str_t* out_json
);

tswn_status_t tswn_score_json(
    const char* raw_text_utf8,
    size_t n,
    const char* mode_utf8,
    uint32_t thread,
    tswn_str_t* out_json
);
tswn_status_t tswn_score_json_with_eval_rq(
    const char* raw_text_utf8,
    size_t n,
    const char* mode_utf8,
    uint32_t thread,
    double eval_rq,
    tswn_str_t* out_json
);

tswn_status_t tswn_namer_pf_json(
    const char* raw_text_utf8,
    size_t n,
    const char* const* modes_utf8,
    size_t modes_len,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);

tswn_status_t tswn_batch_rate_json(
    const char* const* target_groups_utf8,
    size_t target_groups_len,
    const char* const* player_groups_utf8,
    size_t player_groups_len,
    size_t n,
    const char* const* player_labels_utf8,
    size_t player_labels_len,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);

tswn_status_t tswn_pair_rate_json(
    const char* const* target_groups_utf8,
    size_t target_groups_len,
    const char* const* players_utf8,
    size_t players_len,
    const char* const* teammates_utf8,
    size_t teammates_len,
    size_t head,
    size_t n,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);

tswn_status_t tswn_to_diy(const char* name_utf8, uint8_t old, uint8_t minions, tswn_str_t* out_result);

tswn_status_t tswn_batch_rate_factored_json(
    const char* const* target_groups_utf8,
    size_t target_groups_len,
    const double* target_factors,
    size_t target_factors_len,
    const char* const* player_groups_utf8,
    size_t player_groups_len,
    size_t n,
    const char* const* player_labels_utf8,
    size_t player_labels_len,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);

tswn_status_t tswn_pair_rate_factored_json(
    const char* const* target_groups_utf8,
    size_t target_groups_len,
    const double* target_factors,
    size_t target_factors_len,
    const char* const* players_utf8,
    size_t players_len,
    const char* const* teammates_utf8,
    size_t teammates_len,
    size_t head,
    size_t n,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);
tswn_status_t tswn_to_diy_batch_json(
    const char* const* names_utf8,
    size_t names_len,
    uint8_t old,
    uint8_t minions,
    tswn_str_t* out_json
);
tswn_status_t tswn_icon_info_json(const char* name_utf8, tswn_str_t* out_json);
tswn_status_t tswn_parse_group_lines_json(const char* content_utf8, uint8_t double_plus, tswn_str_t* out_json);

/*
 * 使用主 Runtime 跑完整场对局，并返回可直接渲染的结构化回放 JSON。
 * `max_rounds` 必须为正数；建议传入 20000。`include_icons != 0` 时嵌入 PNG Base64 图标。
 * 结果需用 `tswn_str_free` 释放。失败后可通过 tswn_last_error_code/message 查询详情。
 */
tswn_status_t tswn_battle_replay_json(
    const char* raw_text_utf8,
    size_t max_rounds,
    double eval_rq,
    uint8_t include_icons,
    tswn_str_t* out_json
);

/* 返回默认 custom Runtime 的标准化执行轨迹 JSON。结果需用 `tswn_str_free` 释放。 */
tswn_status_t tswn_default_custom_runtime_normalized_run_json(
    const char* raw_text_utf8,
    size_t max_rounds,
    tswn_str_t* out_json
);

/* 图标辅助接口 */

/* 将名字渲染为 16x16 RGBA 原始像素。结果需用 `tswn_bytes_free` 释放。 */
tswn_status_t tswn_name_to_icon_rgba(const char* name_utf8, tswn_bytes_t* out_bytes);

/* 将名字渲染为 PNG 字节。结果需用 `tswn_bytes_free` 释放。 */
tswn_status_t tswn_name_to_png_bytes(const char* name_utf8, tswn_bytes_t* out_bytes);

/* 将名字渲染为 PNG Base64 文本。结果需用 `tswn_str_free` 释放。 */
tswn_str_t tswn_name_to_png_base64(const char* name_utf8);

#ifdef __cplusplus
}
#endif

#endif

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
 * - 新调用方按需取帧用 tswn_battle_session_*，一次回放用 tswn_battle_replay_json
 *
 * 所有权与指针：
 * - 输入仅在调用期间借用；字符串数组必须含声明长度个有效的 NUL 结尾字符串指针
 * - 返回切片不保证 NUL 结尾；按 len 读取，不可把 ptr 直接交给 strlen/printf("%s")
 * - 保留返回的原始 ptr/len，包括零长度非空缓冲；复制描述符不复制分配，只能释放一次
 * - 资源必须由创建它的同一份库释放，不能使用 free/delete；NULL free 为无操作
 * - 非空输出指针指向对齐、可写且容量足够的独立存储，不得与输入或句柄重叠
 * - 复用输出槽前先释放旧资源；非会话接口仅在成功时保证输出可读，失败不保证清零
 * - 句柄是 opaque 独占资源；借用期间不得释放，同一可变句柄不得并发调用
 *
 * 错误边界：
 * - 返回 tswn_status_t 的调用先清旧错误；失败后在同一线程立即读取 last_error_code/message
 * - 错误读取本身不消费错误，每次返回的字符串副本仍须释放
 * - 状态型接口捕获可展开的 Rust panic；失效指针、越界存储、分配失败及 panic=abort 不可恢复
 * - 低层布尔/长度查询不更新错误，兼容消息入口成功不清旧错误；并非全部入口都捕获 panic
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

/* UTF-8 字符串切片，不保证 NUL 结尾。只能读取 len 字节，并以原始 ptr/len 释放。 */
typedef struct tswn_str_t {
    const char* ptr;
    size_t len;
} tswn_str_t;

/* 库分配的只读字节切片；复制结构体不转移唯一释放责任，不能修改 len 后释放。 */
typedef struct tswn_bytes_t {
    const uint8_t* ptr;
    size_t len;
} tswn_bytes_t;

/* 胜场与实际完成的试验数；total 非零时百分比为 wins * 100.0 / total，无需释放。 */
typedef struct tswn_win_rate_result_t {
    uint64_t wins;
    uint64_t total;
} tswn_win_rate_result_t;

/* 玩家属性的值拷贝；ptr 是历史兼容的 id 别名，不是可解引用地址；快照无需释放。 */
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

/* 更新的值拷贝。has_param=0 时忽略 param；target_count 给出 targets_copy 所需容量。 */
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

/* 规范会话 API。raw_text_utf8 按空行分队；max_rounds 为正数 Runtime 回合上限，
 * 不是帧数。include_icons 仅接受 0/1；图标会增加快照和回放数据量。通过追加导出保持 ABI 4。 */
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
/* initial/current 返回独立的规范玩家快照 JSON，不推进会话。
 * next_frame 成功且 has=1 时返回下一帧；正常终止后反复调用均为 has=0。
 * result 仅在正常结束或截断后 has=1；查询不消费结果，重复读取需分别释放字符串。
 * 可选输出先重置为 has=0 和 {NULL,0}，因此调用前必须先释放旧 JSON。
 * status 在 Runtime 失败后仍可能为 RUNNING；is_failed 才是失败判断，STOP_NONE 表示无正常终止原因。 */
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

/* 清除当前线程的错误码和消息；不会修改会话内保存的 sticky Runtime failure。 */
void tswn_clear_error(void);

/* 释放库返回的原始字符串描述符；复制描述符也只能释放一次，非空零长缓冲同样需释放。 */
void tswn_str_free(tswn_str_t value);

/* 释放库返回的原始字节描述符，不能传入其他分配器创建的内存或修改后的长度。 */
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

/* 构造可复用阵容模板，使用普通对局默认 eval_rq；原始输入中的种子不存入模板。 */
tswn_status_t tswn_prepared_runner_new_from_raw(const char* raw_text_utf8, tswn_prepared_runner_t** out_prepared);

/* 从名竞原始输入构造 PreparedRunner，并显式指定 eval_rq。 */
tswn_status_t tswn_prepared_runner_new_from_raw_with_eval_rq(const char* raw_text_utf8, double eval_rq, tswn_prepared_runner_t** out_prepared);

/* 通过 PreparedRunner 和可选 seed 构造 Runner。传入 NULL 表示不带 seed。 */
tswn_status_t tswn_runner_new_from_prepared(const tswn_prepared_runner_t* prepared, const char* seed_utf8, tswn_runner_t** out_runner);

/* Runner 执行 / 状态 */

/* 返回是否已有胜者；NULL 为 0。布尔与长度查询不写 last_error。 */
uint8_t tswn_runner_have_winner(const tswn_runner_t* runner);

/* 同步推进至胜者、无进展或绑定层回合上限；0 不区分截断与 NULL，不捕获 panic。
 * 需要稳定错误码、失败状态和终止原因时使用 tswn_battle_session_*。 */
uint8_t tswn_runner_run_to_completion(tswn_runner_t* runner);

/* 推进一个主回合，返回独立更新句柄，使用 tswn_updates_free 释放。
 * 失败不保证回滚已经推进的 runner；已返回的更新可在 runner 释放后继续读取。 */
tswn_status_t tswn_runner_main_round(tswn_runner_t* runner, tswn_updates_t** out_updates);

/* 返回原始输入顺序对应的队伍数量。 */
size_t tswn_runner_input_group_count(const tswn_runner_t* runner);

/* 返回指定输入队伍的成员数量。 */
size_t tswn_runner_input_group_len(const tswn_runner_t* runner, size_t group_index);

/* 复制零基队伍 roster，容量以 uint64_t 元素个数表示；越界队伍按空列表处理。
 * 所有 *_copy 在容量不足时不复制；空列表允许 NULL 输出，非空列表需足够可写存储。 */
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

/* 返回零基更新的原始消息模板；结果按 len 读取并用 tswn_str_free 释放。
 * NULL/越界返回 {NULL,0} 并记录错误；成功不清旧错误，此兼容入口不捕获 panic。 */
tswn_str_t tswn_updates_message(const tswn_updates_t* updates, size_t index);

/* 返回零基更新的占位符展开消息，所有权/错误边界与 message 一致。 */
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
 * out_results 必须可写 against_len 个结构；零长时数组可为 NULL，失败不回滚已写入的前缀。
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
 * 使用 PreparedRunner 创建时固定的 eval_rq 计算第一组胜率，不会改用全局 win-rate 默认值。
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
 * 复杂返回值统一编码为 JSON UTF-8 字符串，并由调用方使用 tswn_str_free 释放。
 * n 为试验次数；thread=0/1/n 表示自动/单线程/指定线程。raw 按空行分队，
 * team/target/against 元素各表示一个多行队伍；group 结果按 against 输入顺序返回。
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

/* mode 指定单项评分口径（如 normal），返回 score、wins、total、errors 及纳秒耗时。 */
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

/* 每个输入行作为名字组合执行 modes 评分；零长 modes 使用默认模式。
 * keep_rq 非零选择普通构建口径，为零选择胜率评估口径；非空数组中每项均为有效 C 字符串。 */
tswn_status_t tswn_namer_pf_json(
    const char* raw_text_utf8,
    size_t n,
    const char* const* modes_utf8,
    size_t modes_len,
    uint8_t keep_rq,
    uint32_t thread,
    tswn_str_t* out_json
);

/* 候选组逐个对目标组汇总；labels=NULL 使用自动标签，否则长度须与候选数相同。 */
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

/* 评估 players 与 teammates 组合；head 决定用于最终汇总的头部搭档数量。 */
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

/* 返回单名的纯 DIY 文本，不是 JSON；old/minions 非零启用相应格式选项，冲突由 core 校验。 */
tswn_status_t tswn_to_diy(const char* name_utf8, uint8_t old, uint8_t minions, tswn_str_t* out_result);

/* 加权版本的 target_factors 与 target_groups 一一对应，各权重必须有限且为正。 */
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
/* 按输入顺序返回 DIY 字符串数组 JSON；任一名字转换失败则整次调用失败。 */
tswn_status_t tswn_to_diy_batch_json(
    const char* const* names_utf8,
    size_t names_len,
    uint8_t old,
    uint8_t minions,
    tswn_str_t* out_json
);
/* 返回图标形状和颜色索引 JSON，不含 PNG 数据。 */
tswn_status_t tswn_icon_info_json(const char* name_utf8, tswn_str_t* out_json);
/* 逐行解析名字组，double_plus 非零使用 ++ 分隔；返回每组多行文本的 JSON 数组。 */
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

/* 返回 data:image/png;base64,... 文本（含前缀，不是裸 Base64），使用 tswn_str_free 释放。
 * 成功清旧错误，失败返回 {NULL,0}；NULL/非法 UTF-8 分别为 INVALID_ARGUMENT/INVALID_INPUT。
 * 此入口捕获可展开的 panic，记录 INTERNAL_ERROR；错误详情仍由同线程 last_error 查询。 */
tswn_str_t tswn_name_to_png_base64(const char* name_utf8);

#ifdef __cplusplus
}
#endif

#endif

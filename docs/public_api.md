# 对外 API 对齐约定

`tswn_capi`、`tswn_py` 与 `tswn_wasm` 的推荐调用面是同一组用户级功能：

- 胜率摘要：`win_rate_summary`、`team_win_rate_summary`、`group_win_rate_summary`
- 评分：`score`、`namer_pf`、`batch_rate`、`pair_rate`
- 工具：`to_diy`、`to_diy_batch`、`icon_info`、`parse_group_lines`
- 回放：`battle_replay`
- 诊断轨迹：`default_custom_runtime_normalized_run`

三端可按各自习惯返回 Python 对象、WASM 对象或 C JSON；字段使用 snake_case，含义、默认 `eval_rq` 和输入校验由 `tswn_core::cli_api` 统一决定。C 的 JSON 字符串由 `tswn_str_free` 释放。

`battle_replay` 是推荐的一次性对局入口。它统一返回初始/最终状态、逐回合 updates、可直接渲染的 rows/clips/parts、赢家信息与完成状态。默认最大回合数为 20,000；达到限制时返回 `finished=false, truncated=true` 的正常结果。

错误的稳定 code 为 `INVALID_INPUT`、`INVALID_ARGUMENT`、`UNSUPPORTED_OPTION`、`RUNNER_INIT_FAILED`、`RUNTIME_FAILED` 和 `INTERNAL_ERROR`。WASM 错误直接包含 `{ code, message }`；C 使用 status 并通过 `tswn_last_error_code()` / `tswn_last_error_message()` 查询详情。

`Runner`、`PreparedRunner`、`FightSession`、`WinRateSession`、RC4 与基础 `win_rate` 属于 Advanced API：仍可使用，但返回形式与会话能力允许语言化，不作为三端功能对齐承诺。PreparedRunner 的 `eval_rq` 在创建时固定，不能在计算胜率时覆盖。

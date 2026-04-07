#include "common.h"

/*
 * 说明：
 * - 这个示例演示如何先构造一份 PreparedRunner，再直接调用 C-API 提供的高层胜率接口。
 * - `thread` 参数统一约定为：0=自动线程数，1=单线程，n=指定线程数。
 * - 若你自己走 `tswn_runner_new_from_prepared(...)` 路径，seed 仍应传完整 `seed:...` 行。
 */

int main(void) {
    /*
     * 这里仍然使用 raw 文本来定义参战双方，
     * 但只 prepare 一次，后续重复构造 runner 并统计胜率。
     */
    const char* raw = "114514\n\n1919810";
    const uint32_t thread = 4; /* 0=自动线程数, 1=单线程, n=指定线程数 */
    tswn_prepared_runner_t* prepared = NULL;
    tswn_win_rate_result_t result;

    if (!tswn_example_require(
            tswn_prepared_runner_new_from_raw(raw, &prepared),
            "prepare failed")) {
        return 1;
    }

    if (!tswn_example_require(
            tswn_prepared_win_rate(prepared, 1000, thread, &result),
            "prepared_win_rate failed")) {
        tswn_prepared_runner_free(prepared);
        return 1;
    }

    printf("prepared wins=%llu total=%llu rate=%.2f%%\n",
           (unsigned long long)result.wins,
           (unsigned long long)result.total,
           tswn_example_percent(result));

    tswn_prepared_runner_free(prepared);
    return 0;
}

#include "common.h"

int main(void) {
    const char* raw = "114514\n\n1919810";
    tswn_win_rate_result_t result;

    if (!tswn_example_require(tswn_win_rate(raw, 1000, &result), "win_rate failed")) {
        return 1;
    }
    printf("default wins=%llu total=%llu rate=%.2f%%\n",
           (unsigned long long)result.wins,
           (unsigned long long)result.total,
           tswn_example_percent(result));

    if (!tswn_example_require(
            tswn_win_rate_with_eval_rq(raw, 1000, tswn_default_eval_rq(), &result),
            "win_rate_with_eval_rq failed")) {
        return 1;
    }
    printf("default_eval_rq wins=%llu total=%llu rate=%.2f%%\n",
           (unsigned long long)result.wins,
           (unsigned long long)result.total,
           tswn_example_percent(result));

    return 0;
}

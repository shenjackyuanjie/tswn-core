#include "common.h"

int main(void) {
    const char* target = "114514";
    const char* against[] = {"1919810", "aaa", "bbb"};
    tswn_win_rate_result_t results[3];

    if (!tswn_example_require(
            tswn_group_win_rate(target, against, 3, 1000, results),
            "group_win_rate failed")) {
        return 1;
    }

    for (size_t i = 0; i < 3; ++i) {
        printf("vs %s wins=%llu total=%llu rate=%.2f%%\n",
               against[i],
               (unsigned long long)results[i].wins,
               (unsigned long long)results[i].total,
               tswn_example_percent(results[i]));
    }

    return 0;
}

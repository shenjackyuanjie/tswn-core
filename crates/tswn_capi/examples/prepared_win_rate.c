#include "common.h"

/*
 * 说明：
 * - 这个示例演示如何先构造一份 PreparedRunner，再基于它批量统计胜率。
 * - 当前 C-API 中，prepared 路径的 seed 应传“完整 seed 行”，例如：
 *     "seed:33554431@!"
 *   而不是只传：
 *     "33554431@!"
 * - 若传 NULL，则表示本局不带 seed。
 */

static int prepared_win_rate(
    tswn_prepared_runner_t* prepared,
    size_t n,
    tswn_win_rate_result_t* out_result
) {
    if (prepared == NULL || out_result == NULL) {
        return 0;
    }

    out_result->wins = 0;
    out_result->total = 0;

    for (size_t i = 0; i < n; ++i) {
        tswn_runner_t* runner = NULL;
        char seed_buf[64];
        const char* seed = NULL;

        if (i != 0) {
            /* 与当前 win-rate 语义保持一致：首局无 seed，后续使用完整 seed 行。 */
            snprintf(seed_buf, sizeof(seed_buf), "seed:%zu@!", (size_t)(33554430 + i));
            seed = seed_buf;
        }

        if (!tswn_example_require(
                tswn_runner_new_from_prepared(prepared, seed, &runner),
                "runner from prepared failed")) {
            return 0;
        }

        if (!tswn_runner_run_to_completion(runner)) {
            tswn_runner_free(runner);
            tswn_example_print_error("run_to_completion failed");
            return 0;
        }

        {
            size_t winner_len = tswn_runner_winner_len(runner);
            size_t team0_len = tswn_runner_input_group_len(runner, 0);
            uint64_t* winners = NULL;
            uint64_t* team0 = NULL;
            int team0_win = 0;

            if (winner_len > 0) {
                winners = (uint64_t*)malloc(sizeof(uint64_t) * winner_len);
                if (winners == NULL) {
                    tswn_runner_free(runner);
                    fprintf(stderr, "malloc winners failed\n");
                    return 0;
                }
                if (!tswn_example_require(
                        tswn_runner_winner_copy(runner, winners, winner_len),
                        "winner copy failed")) {
                    free(winners);
                    tswn_runner_free(runner);
                    return 0;
                }
            }

            if (team0_len > 0) {
                team0 = (uint64_t*)malloc(sizeof(uint64_t) * team0_len);
                if (team0 == NULL) {
                    free(winners);
                    tswn_runner_free(runner);
                    fprintf(stderr, "malloc team0 failed\n");
                    return 0;
                }
                if (!tswn_example_require(
                        tswn_runner_input_group_copy(runner, 0, team0, team0_len),
                        "team0 input group copy failed")) {
                    free(team0);
                    free(winners);
                    tswn_runner_free(runner);
                    return 0;
                }
            }

            for (size_t wi = 0; wi < winner_len && !team0_win; ++wi) {
                for (size_t ti = 0; ti < team0_len; ++ti) {
                    if (winners[wi] == team0[ti]) {
                        team0_win = 1;
                        break;
                    }
                }
            }

            if (team0_win) {
                out_result->wins += 1;
            }
            out_result->total += 1;

            free(team0);
            free(winners);
        }

        tswn_runner_free(runner);
    }

    return 1;
}

int main(void) {
    /*
     * 这里仍然使用 raw 文本来定义参战双方，
     * 但只 prepare 一次，后续重复构造 runner 并统计胜率。
     */
    const char* raw = "114514\n\n1919810";
    tswn_prepared_runner_t* prepared = NULL;
    tswn_win_rate_result_t result;

    if (!tswn_example_require(
            tswn_prepared_runner_new_from_raw(raw, &prepared),
            "prepare failed")) {
        return 1;
    }

    if (!prepared_win_rate(prepared, 1000, &result)) {
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
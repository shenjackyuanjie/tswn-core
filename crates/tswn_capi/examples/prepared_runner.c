#include "common.h"

/*
 * 注意：
 * - tswn_runner_new_from_prepared() 的 seed 参数应传“完整 seed 行”
 * - 也就是像 "seed:33554431@!" 这样的字符串
 * - 而不是只传 "33554431@!"
 *
 * 这样才能与 raw 输入里的 seed 语义保持一致。
 */
static void run_with_seed(tswn_prepared_runner_t* prepared, const char* seed) {
    tswn_runner_t* runner = NULL;
    if (!tswn_example_require(tswn_runner_new_from_prepared(prepared, seed, &runner), "runner from prepared failed")) {
        return;
    }
    tswn_runner_run_to_completion(runner);
    printf("seed=%s winner_len=%zu\n", seed == NULL ? "<none>" : seed, tswn_runner_winner_len(runner));
    tswn_runner_free(runner);
}

int main(void) {
    const char* raw = "114514\n\n1919810";
    tswn_prepared_runner_t* prepared = NULL;
    if (!tswn_example_require(tswn_prepared_runner_new_from_raw(raw, &prepared), "prepare failed")) {
        return 1;
    }

    run_with_seed(prepared, NULL);
    /* 传入完整 seed 行，而不是裸 seed 值。 */
    run_with_seed(prepared, "seed:33554431@!");
    run_with_seed(prepared, "seed:33554432@!");

    tswn_prepared_runner_free(prepared);
    return 0;
}

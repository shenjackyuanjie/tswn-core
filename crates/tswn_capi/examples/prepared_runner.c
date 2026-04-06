#include "common.h"

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
    const char* raw = "SB\n\nLJ";
    tswn_prepared_runner_t* prepared = NULL;
    if (!tswn_example_require(tswn_prepared_runner_new_from_raw(raw, &prepared), "prepare failed")) {
        return 1;
    }

    run_with_seed(prepared, NULL);
    run_with_seed(prepared, "seed:33554431@!");
    run_with_seed(prepared, "seed:33554432@!");

    tswn_prepared_runner_free(prepared);
    return 0;
}

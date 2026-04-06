#include "common.h"

int main(void) {
    const char* raw = "SB\n\nLJ\nseed:33554431@!";
    tswn_runner_t* runner = NULL;
    if (!tswn_example_require(tswn_runner_new_from_raw(raw, &runner), "create runner failed")) {
        return 1;
    }

    printf("have_winner_before=%u\n", tswn_runner_have_winner(runner));
    printf("completed=%u\n", tswn_runner_run_to_completion(runner));

    size_t winner_len = tswn_runner_winner_len(runner);
    uint64_t* winner_ids = winner_len == 0 ? NULL : (uint64_t*)malloc(sizeof(uint64_t) * winner_len);
    if (!tswn_example_require(tswn_runner_winner_copy(runner, winner_ids, winner_len), "winner copy failed")) {
        free(winner_ids);
        tswn_runner_free(runner);
        return 1;
    }

    printf("winner_ids=");
    for (size_t i = 0; i < winner_len; ++i) {
        printf(i == 0 ? "%llu" : ",%llu", (unsigned long long)winner_ids[i]);
    }
    printf("\n");

    free(winner_ids);
    tswn_runner_free(runner);
    return 0;
}

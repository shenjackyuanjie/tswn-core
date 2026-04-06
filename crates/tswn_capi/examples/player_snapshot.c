#include "common.h"

int main(void) {
    const char* raw = "114514\n\n1919810\nseed:33554431@!";
    tswn_runner_t* runner = NULL;
    if (!tswn_example_require(tswn_runner_new_from_raw(raw, &runner), "create runner failed")) {
        return 1;
    }

    size_t count = tswn_runner_all_player_count(runner);
    uint64_t* ids = count == 0 ? NULL : (uint64_t*)malloc(sizeof(uint64_t) * count);
    if (!tswn_example_require(tswn_runner_all_player_ids_copy(runner, ids, count), "player id copy failed")) {
        free(ids);
        tswn_runner_free(runner);
        return 1;
    }

    for (size_t i = 0; i < count; ++i) {
        tswn_player_snapshot_t player;
        if (!tswn_example_require(tswn_runner_player_snapshot(runner, ids[i], &player), "player snapshot failed")) {
            break;
        }
        printf("id=%llu hp=%d/%d atk=%d def=%d all_sum=%u\n",
               (unsigned long long)player.id,
               player.hp,
               player.max_hp,
               player.attack,
               player.defense,
               player.all_sum);
    }

    free(ids);
    tswn_runner_free(runner);
    return 0;
}

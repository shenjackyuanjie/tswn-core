#include "common.h"

static const char* tswn_example_status_name(tswn_status_t status) {
    switch (status) {
        case TSWN_OK:
            return "TSWN_OK";
        case TSWN_ERR_NULL:
            return "TSWN_ERR_NULL";
        case TSWN_ERR_INVALID_UTF8:
            return "TSWN_ERR_INVALID_UTF8";
        case TSWN_ERR_INVALID_ARGUMENT:
            return "TSWN_ERR_INVALID_ARGUMENT";
        case TSWN_ERR_RUNNER:
            return "TSWN_ERR_RUNNER";
        case TSWN_ERR_PANIC:
            return "TSWN_ERR_PANIC";
        default:
            return "TSWN_ERR_UNKNOWN";
    }
}

static void tswn_example_dump_last_error(const char* label) {
    tswn_str_t err = tswn_last_error_message();
    if (err.ptr != NULL && err.len > 0) {
        printf("%s: %.*s\n", label, (int)err.len, err.ptr);
    } else {
        printf("%s: <empty>\n", label);
    }
    tswn_str_free(err);
}

static void tswn_example_print_status(const char* label, tswn_status_t status) {
    printf("%s: status=%u (%s)\n", label, (unsigned)status, tswn_example_status_name(status));
}

int main(void) {
    const char* raw = "114514\n\n1919810";
    tswn_runner_t* runner = NULL;
    tswn_player_snapshot_t snapshot;
    tswn_status_t status;
    size_t player_count = 0;
    uint64_t* player_ids = NULL;

    /* 场景 1：传入 NULL，读取并手动清理 last_error。 */
    status = tswn_runner_new_from_raw(NULL, &runner);
    tswn_example_print_status("case1/null raw", status);
    tswn_example_dump_last_error("case1/last_error");
    tswn_clear_error();
    tswn_example_dump_last_error("case1/after_clear");

    /* 先构造一个正常 runner，后面再演示“失败后继续调用”。 */
    if (!tswn_example_require(tswn_runner_new_from_raw(raw, &runner), "create runner failed")) {
        return 1;
    }

    player_count = tswn_runner_all_player_count(runner);
    if (player_count == 0) {
        fprintf(stderr, "unexpected: player_count == 0\n");
        tswn_runner_free(runner);
        return 1;
    }
    player_ids = (uint64_t*)malloc(sizeof(uint64_t) * player_count);
    if (player_ids == NULL) {
        fprintf(stderr, "malloc player_ids failed\n");
        tswn_runner_free(runner);
        return 1;
    }
    if (!tswn_example_require(
            tswn_runner_all_player_ids_copy(runner, player_ids, player_count),
            "copy all player ids failed")) {
        free(player_ids);
        tswn_runner_free(runner);
        return 1;
    }

    /* 场景 2：传入不存在的 player_id，读取 last_error。 */
    status = tswn_runner_player_snapshot(runner, 999999999ULL, &snapshot);
    tswn_example_print_status("case2/missing player", status);
    tswn_example_dump_last_error("case2/last_error");

    /* 场景 3：错误发生后继续正常调用；成功调用会清掉旧错误。 */
    status = tswn_runner_player_snapshot(runner, player_ids[0], &snapshot);
    tswn_example_print_status("case3/real player", status);
    if (status == TSWN_OK) {
         printf("case3/snapshot: id=%llu hp=%d/%d magic_point=%d all_sum=%d\n",
               (unsigned long long)snapshot.id,
               snapshot.hp,
               snapshot.max_hp,
             snapshot.magic_point,
               snapshot.all_sum);
    }
    tswn_example_dump_last_error("case3/last_error_after_success");

    free(player_ids);
    tswn_runner_free(runner);
    return 0;
}

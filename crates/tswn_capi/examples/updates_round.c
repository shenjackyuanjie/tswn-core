#include "common.h"

int main(void) {
    const char* raw = "114514\n\n1919810\nseed:33554431@!";
    tswn_runner_t* runner = NULL;
    tswn_updates_t* updates = NULL;
    if (!tswn_example_require(tswn_runner_new_from_raw(raw, &runner), "create runner failed")) {
        return 1;
    }
    if (!tswn_example_require(tswn_runner_main_round(runner, &updates), "main_round failed")) {
        tswn_runner_free(runner);
        return 1;
    }

    size_t len = tswn_updates_len(updates);
    printf("updates=%zu\n", len);
    for (size_t i = 0; i < len; ++i) {
        tswn_update_snapshot_t update;
        if (!tswn_example_require(tswn_updates_get(updates, i, &update), "updates_get failed")) {
            break;
        }
        tswn_str_t raw_msg = tswn_updates_message(updates, i);
        tswn_str_t rendered = tswn_updates_message_rendered(updates, i);
        printf("[%zu] type=%u score=%u raw=%.*s rendered=%.*s\n",
               i,
               (unsigned)update.update_type,
               update.score,
               (int)raw_msg.len,
               raw_msg.ptr,
               (int)rendered.len,
               rendered.ptr);
        tswn_str_free(raw_msg);
        tswn_str_free(rendered);
    }

    tswn_updates_free(updates);
    tswn_runner_free(runner);
    return 0;
}

#include "common.h"

int main(void) {
    tswn_str_t version = tswn_version();
    printf("abi=%u\n", tswn_capi_abi_version());
    printf("version=%.*s\n", (int)version.len, version.ptr);
    printf("default_eval_rq=%.1f\n", tswn_default_eval_rq());
    printf("win_rate_eval_rq=%.1f\n", tswn_win_rate_eval_rq());
    tswn_str_free(version);

    tswn_runner_t* runner = NULL;
    if (!tswn_example_require(tswn_runner_new_from_raw(NULL, &runner), "expected error")) {
        tswn_clear_error();
    }

    return 0;
}

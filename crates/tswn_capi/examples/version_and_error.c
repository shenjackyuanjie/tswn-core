#include "common.h"

int main(void) {
    tswn_str_t capi_version = tswn_capi_version();
    tswn_str_t core_version = tswn_core_version();
    printf("abi=%u\n", tswn_capi_abi_version());
    printf("capi_version=%.*s\n", (int)capi_version.len, capi_version.ptr);
    printf("core_version=%.*s\n", (int)core_version.len, core_version.ptr);
    printf("default_eval_rq=%.1f\n", tswn_default_eval_rq());
    printf("win_rate_eval_rq=%.1f\n", tswn_win_rate_eval_rq());
    tswn_str_free(capi_version);
    tswn_str_free(core_version);

    tswn_runner_t* runner = NULL;
    if (!tswn_example_require(tswn_runner_new_from_raw(NULL, &runner), "expected error")) {
        tswn_clear_error();
    }

    return 0;
}

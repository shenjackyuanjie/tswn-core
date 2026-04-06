#ifndef TSWN_CAPI_EXAMPLES_COMMON_H
#define TSWN_CAPI_EXAMPLES_COMMON_H

#include <stdio.h>
#include <stdlib.h>

#include "../include/tswn_capi.h"

static void tswn_example_print_error(const char* prefix) {
    tswn_str_t err = tswn_last_error_message();
    if (err.ptr != NULL && err.len > 0) {
        fprintf(stderr, "%s: %.*s\n", prefix, (int)err.len, err.ptr);
    } else {
        fprintf(stderr, "%s\n", prefix);
    }
    tswn_str_free(err);
}

static int tswn_example_require(tswn_status_t status, const char* prefix) {
    if (status == TSWN_OK) {
        return 1;
    }
    tswn_example_print_error(prefix);
    return 0;
}

static double tswn_example_percent(tswn_win_rate_result_t result) {
    return result.total == 0 ? 0.0 : (double)result.wins * 100.0 / (double)result.total;
}

#endif

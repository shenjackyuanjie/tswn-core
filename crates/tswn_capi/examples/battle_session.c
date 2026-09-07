#include "common.h"

/* Stream canonical JSON payloads; every returned string is freed immediately. */
int main(void) {
    tswn_battle_options_t options;
    tswn_battle_session_t* session = NULL;
    tswn_str_t json = {NULL, 0};
    uint8_t has = 0;
    int result = 1;
    if (tswn_capi_abi_version() != 4) return 2;
    tswn_battle_options_default(&options);
    if (!tswn_example_require(tswn_battle_session_new("left@red\n\nright@blue", &options, &session), "new")) goto cleanup;
    if (!tswn_example_require(tswn_battle_session_initial_states_json(session, &json), "initial")) goto cleanup;
    printf("%.*s\n", (int)json.len, json.ptr);
    tswn_str_free(json);
    for (;;) {
        if (!tswn_example_require(tswn_battle_session_next_frame_json(session, &has, &json), "frame")) goto cleanup;
        if (!has) break;
        printf("%.*s\n", (int)json.len, json.ptr);
        tswn_str_free(json);
    }
    if (!tswn_example_require(tswn_battle_session_result_json(session, &has, &json), "result")) goto cleanup;
    if (!has) goto cleanup;
    printf("%.*s\n", (int)json.len, json.ptr);
    tswn_str_free(json);
    result = 0;
cleanup:
    tswn_battle_session_free(session);
    return result;
}

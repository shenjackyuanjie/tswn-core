#include "common.h"

int main(void) {
    tswn_bytes_t rgba;
    tswn_bytes_t png;
    tswn_str_t b64;

    if (!tswn_example_require(tswn_name_to_icon_rgba("SB", &rgba), "icon rgba failed")) {
        return 1;
    }
    if (!tswn_example_require(tswn_name_to_png_bytes("SB", &png), "icon png failed")) {
        tswn_bytes_free(rgba);
        return 1;
    }
    b64 = tswn_name_to_png_base64("SB");
    if (b64.ptr == NULL) {
        tswn_example_print_error("icon b64 failed");
        tswn_bytes_free(rgba);
        tswn_bytes_free(png);
        return 1;
    }

    printf("rgba_len=%zu\n", rgba.len);
    printf("png_len=%zu\n", png.len);
    printf("b64_prefix=%.*s\n", b64.len > 32 ? 32 : (int)b64.len, b64.ptr);

    tswn_bytes_free(rgba);
    tswn_bytes_free(png);
    tswn_str_free(b64);
    return 0;
}

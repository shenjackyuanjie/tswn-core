//! 迁移 DS3_demo3 时的 C++ 行为兼容性说明。
//!
//! - `duplicate.cpp` 保留原始行文本，并将 `\r` / `\n` 视为分隔符。
//! - `sort.cpp` 按所选分数降序、再按名称降序排序。
//! - `all.cpp` 写入包含 `1@1` 的 `tmp/blank.txt`。

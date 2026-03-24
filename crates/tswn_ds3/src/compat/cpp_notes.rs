//! Notes for C++ behavior compatibility while migrating DS3_demo3.
//!
//! - `duplicate.cpp` keeps raw line text and treats `\r` / `\n` as separators.
//! - `sort.cpp` sorts by selected score descending, then name descending.
//! - `all.cpp` writes `tmp/blank.txt` containing `1@1`.

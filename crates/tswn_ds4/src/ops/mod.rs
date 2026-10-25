//! 评分文件操作子模块集合。
//!
//! `merge` 负责拼接新增输入的原始字节，`dedup` 按完整原始行排除历史重复，
//! `sort` 负责按指定分数列重排并输出。它们组成 DS4 pipeline 中评分前后的文件整理阶段。

pub mod dedup;
pub mod merge;
pub mod sort;

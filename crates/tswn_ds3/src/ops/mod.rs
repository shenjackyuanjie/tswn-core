//! 评分文件操作子模块集合。
//!
//! `merge` 负责合并新增输入，`dedup` 负责删除已排序文件中的重复名字，
//! `sort` 负责按指定分数列重排并输出。它们组成 DS3 pipeline 中评分前后的文件整理阶段。

pub mod dedup;
pub mod merge;
pub mod sort;

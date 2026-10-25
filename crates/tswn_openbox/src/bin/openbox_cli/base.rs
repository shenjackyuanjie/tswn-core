//! `openbox-cli base`：输出名字的原始 128 字节 roll、属性与 16 个技能槽。
//!
//! 渲染逻辑在 `tswn_openbox_backend::backend::base`，与 GUI 的 base 页共用同一份
//! 实现，因此这条子命令的输出和界面日志逐字节一致；这里只保留命令层的入口别名。

pub use tswn_openbox::backend::run_base as run;

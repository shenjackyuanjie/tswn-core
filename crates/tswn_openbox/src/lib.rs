//! tswn openbox 的库入口。
//!
//! - [`backend`]：四个工具的后端实现（解析、执行、输出格式化），不依赖任何
//!   GUI 框架，GUI（`app`）与 CLI（`bin/openbox_cli`）共用同一套逻辑。
//! - [`presets`]：`setting/settings.toml` 预设与默认资源释放，同样与 GUI 无关。

pub mod backend;
pub mod presets;

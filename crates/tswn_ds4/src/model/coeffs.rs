//! 单字与配对流程共用的五套模型系数；按模型分文件，不重复保存数据。
//!
//! 每套固定 1124 项，依次对应常数、46 个线性项及带四处省略项的二次上三角。
//! 位置解释由 engine 的索引表统一定义，不能仅重排系数字面量或更换特征顺序。

pub const MODEL_SIZE: usize = 1124;

mod bc;
pub use bc::MODEL_BC;

mod fz;
pub use fz::MODEL_FZ;

mod wc;
pub use wc::MODEL_WC;

mod fs;
pub use fs::MODEL_FS;

mod pj;
pub use pj::MODEL_PJ;

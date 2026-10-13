//! 单字与配对流程共用的五套模型系数；按模型分文件，不重复保存数据。

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

//! 从 Data_Structure4.0/src/SP1.cpp 提取的 SP1 模型系数，按模型分文件。

#![allow(clippy::excessive_precision)]

mod qp0;
pub use qp0::MODEL_QP0;

mod qd0;
pub use qd0::MODEL_QD0;

mod pp0;
pub use pp0::MODEL_PP0;

mod pd0;
pub use pd0::MODEL_PD0;

mod cqd0;
pub use cqd0::MODEL_CQD0;

mod cqd3;
pub use cqd3::MODEL_CQD3;

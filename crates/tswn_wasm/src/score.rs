use tswn_core::runtime::ScoreTestSession as CoreSession;
use wasm_bindgen::prelude::*;

use crate::battle::dto_to_js;
use crate::error::{WasmResult, cli_api_error};

#[wasm_bindgen(typescript_custom_section)]
/// 评分测试导出类型的 TypeScript 声明（自定义 section）。
const SCORE_TEST_TYPES: &str = r#"
export interface ScoreSkillFrequency {
    message: string;
    count: number;
    frequency: number;
}

export interface ScoreTestProgress {
    completed: number;
    total: number;
    wins: number;
    score: number;
    truncated: number;
    done: boolean;
    skills: ScoreSkillFrequency[];
}
"#;

#[wasm_bindgen]
/// 可步进的 10000 场交互评分。宿主负责调度与释放，长批次建议在 Worker 中执行。
pub struct ScoreTestSession {
    inner: CoreSession,
}

#[wasm_bindgen]
impl ScoreTestSession {
    #[wasm_bindgen(constructor)]
    /// 传入剥掉指令后的单队名字文本；enhanced 默认 false，仅控制陪测角色。
    pub fn new(raw_input: String, enhanced: Option<bool>) -> WasmResult<Self> {
        crate::install_panic_hook();
        Ok(Self {
            inner: CoreSession::new(&raw_input, enhanced.unwrap_or(false)).map_err(|error| cli_api_error(error.into()))?,
        })
    }

    /// 同步推进至多 batch_size 场，场次数达到 10000 后不再推进。
    pub fn step(&mut self, batch_size: usize) -> WasmResult<()> {
        self.inner.step(batch_size).map_err(|error| cli_api_error(error.into()))
    }

    #[wasm_bindgen(unchecked_return_type = "ScoreTestProgress")]
    /// 复制当前累计评分和技能／动作频率。
    pub fn progress(&self) -> WasmResult<JsValue> { dto_to_js(&self.inner.progress()) }
}

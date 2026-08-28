use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct NameRow {
    pub id: i64,
    pub raw: String,
    pub text_type: String,
}

#[derive(Debug, Clone)]
pub struct TargetRow {
    pub weight: f64,
    pub raw: String,
    pub left_name: String,
    pub right_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultRow {
    pub rank: usize,
    pub score: f64,
    pub text_type: String,
    pub name: String,
    pub coefficient: f64,
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultPartnerRow {
    pub rank: usize,
    pub win_rate: f64,
    pub text_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultDetailRow {
    #[serde(flatten)]
    pub result: ResultRow,
    pub partners: Vec<ResultPartnerRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub pair_done: usize,
    pub pair_total: usize,
    pub target_done: usize,
    pub target_total: usize,
    pub iteration: usize,
    pub message: String,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            pair_done: 0,
            pair_total: 0,
            target_done: 0,
            target_total: 0,
            iteration: 0,
            message: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TextRequest {
    pub text: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct RecomputeRequest {
    pub outer_workers: Option<usize>,
    pub skip_archived: Option<bool>,
}

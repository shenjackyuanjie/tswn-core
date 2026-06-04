use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use tower_http::services::ServeDir;

use crate::model::{
    AddGroupsRequest, BlockGroupRequest, BlockGroupsByTextRequest, ConstrainedSelectionRequest, MergeTeamsRequest,
    PurgeLowScoreGroupsRequest, RecomputeLaneRequest,
};
use crate::service::AppService;

pub type SharedService = Arc<AppService>;

pub fn router(service: AppService) -> Router {
    let shared = Arc::new(service);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/groups/add", post(add_groups))
        .route("/api/groups/block", post(block_groups_by_text))
        .route("/api/groups/unblock", post(unblock_groups_by_text))
        .route("/api/groups/:group_id/block", post(block_group))
        .route("/api/groups/:group_id/unblock", post(unblock_group))
        .route("/api/teams/merge", post(merge_teams))
        .route("/api/lanes", get(lanes))
        .route("/api/lanes/:lane_size/results", get(lane_results))
        .route("/api/lanes/:lane_size/progress", get(lane_progress))
        .route("/api/lanes/:lane_size/recompute", post(recompute_lane))
        .route("/api/lanes/:lane_size/purge-low-score", post(purge_low_score_groups))
        .route("/api/lanes/:lane_size/constrained-selection", post(constrained_selection))
        .route("/api/jobs/:job_id", get(job))
        .nest_service("/", ServeDir::new("crates/tswn_lane_ranker/static"))
        .with_state(shared)
}

async fn health() -> Json<serde_json::Value> { Json(json!({ "ok": true })) }

async fn add_groups(
    State(service): State<SharedService>,
    Json(req): Json<AddGroupsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.add_groups(req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn block_groups_by_text(
    State(service): State<SharedService>,
    Json(req): Json<BlockGroupsByTextRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.set_groups_blocked_by_text(true, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn unblock_groups_by_text(
    State(service): State<SharedService>,
    Json(req): Json<BlockGroupsByTextRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.set_groups_blocked_by_text(false, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn block_group(
    State(service): State<SharedService>,
    Path(group_id): Path<i64>,
    Json(req): Json<BlockGroupRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.set_group_blocked(group_id, true, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn unblock_group(
    State(service): State<SharedService>,
    Path(group_id): Path<i64>,
    Json(req): Json<BlockGroupRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.set_group_blocked(group_id, false, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn merge_teams(
    State(service): State<SharedService>,
    Json(req): Json<MergeTeamsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let response = service.merge_teams(req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn recompute_lane(
    State(service): State<SharedService>,
    Path(lane_size): Path<usize>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let req = if body.is_empty() {
        RecomputeLaneRequest {
            stickiness: None,
            outer_workers: None,
            inner_workers: None,
            skip_archived: None,
        }
    } else {
        serde_json::from_slice::<RecomputeLaneRequest>(&body)?
    };

    if matches!(req.stickiness, Some(0)) {
        return Err(anyhow::anyhow!("stickiness must be a positive integer").into());
    }

    let response = service.queue_recompute_lane(
        lane_size,
        req.stickiness,
        req.outer_workers,
        req.inner_workers,
        req.skip_archived,
    )?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn purge_low_score_groups(
    State(service): State<SharedService>,
    Path(lane_size): Path<usize>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let req = if body.is_empty() {
        PurgeLowScoreGroupsRequest {
            outer_workers: None,
            inner_workers: None,
            skip_archived: None,
        }
    } else {
        serde_json::from_slice::<PurgeLowScoreGroupsRequest>(&body)?
    };

    let response = service.purge_low_score_groups(lane_size, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn constrained_selection(
    State(service): State<SharedService>,
    Path(lane_size): Path<usize>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let req = if body.is_empty() {
        ConstrainedSelectionRequest {
            outer_workers: None,
            inner_workers: None,
            cqd_threshold: None,
        }
    } else {
        serde_json::from_slice::<ConstrainedSelectionRequest>(&body)?
    };

    let response = service.queue_constrained_selection_lane(lane_size, req)?;
    Ok(Json(serde_json::to_value(response)?))
}

async fn lanes(State(service): State<SharedService>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(service.db.lane_statuses()?)?))
}

async fn lane_results(
    State(service): State<SharedService>,
    Path(lane_size): Path<usize>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(service.db.lane_results(lane_size)?)?))
}

async fn lane_progress(
    State(service): State<SharedService>,
    Path(lane_size): Path<usize>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(service.db.lane_progress(lane_size)?)?))
}

async fn job(State(service): State<SharedService>, Path(job_id): Path<i64>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(service.db.job(job_id)?)?))
}

#[derive(Debug)]
pub struct ApiError(anyhow::Error);

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self { Self(err.into()) }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.0.to_string()
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

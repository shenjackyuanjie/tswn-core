use crate::{
    model::{RecomputeRequest, TextRequest},
    service::Service,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use tower_http::services::ServeDir;
pub fn router(service: Service) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({"ok":true})) }))
        .route("/api/names", get(names))
        .route("/api/names/add", post(add_names))
        .route("/api/expansions", get(expansions))
        .route("/api/expansions/add", post(add_expansions))
        .route("/api/expansions/remove", post(remove_expansions))
        .route("/api/measure-one", post(measure_one))
        .route("/api/targets/import", post(import_targets))
        .route("/api/recompute", post(recompute))
        .route("/api/status", get(status))
        .route("/api/results", get(results))
        .route("/api/results/export", get(export_results))
        .nest_service("/", ServeDir::new(concat!(env!("CARGO_MANIFEST_DIR"), "/static")))
        .with_state(Arc::new(service))
}
async fn names(State(s): State<Arc<Service>>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(
        s.db.names()?.into_iter().map(|x| x.0).collect::<Vec<_>>(),
    )?))
}
async fn add_names(State(s): State<Arc<Service>>, Json(r): Json<TextRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"added":s.add_names(&r.text)?})))
}
async fn expansions(State(s): State<Arc<Service>>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"names":s.db.expanded_names()?})))
}
async fn add_expansions(State(s): State<Arc<Service>>, Json(r): Json<TextRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"changed":s.set_names_expanded(&r.text,true)?})))
}
async fn remove_expansions(
    State(s): State<Arc<Service>>,
    Json(r): Json<TextRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"changed":s.set_names_expanded(&r.text,false)?})))
}
async fn measure_one(State(s): State<Arc<Service>>, Json(r): Json<TextRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"measured": s.measure_one(&r.text)?})))
}
async fn import_targets(State(s): State<Arc<Service>>, Json(r): Json<TextRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({"imported":s.import_targets(&r.text)?})))
}
async fn recompute(State(s): State<Arc<Service>>, body: Bytes) -> Result<Json<serde_json::Value>, ApiError> {
    let r = if body.is_empty() {
        RecomputeRequest::default()
    } else {
        serde_json::from_slice(&body)?
    };
    s.queue(r)?;
    Ok(Json(json!({"queued":true})))
}
async fn status(State(s): State<Arc<Service>>) -> Json<crate::model::Status> { Json(s.status.lock().unwrap().clone()) }
async fn results(State(s): State<Arc<Service>>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::to_value(s.result_details()?)?))
}
async fn export_results(State(s): State<Arc<Service>>) -> Result<impl IntoResponse, ApiError> {
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=tswn_name_ranker_details.txt",
            ),
        ],
        s.export_results()?,
    ))
}
pub struct ApiError(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self { Self(e.into()) }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":self.0.to_string()}))).into_response()
    }
}

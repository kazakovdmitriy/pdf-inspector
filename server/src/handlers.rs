//! HTTP handlers for the convert / detect / health endpoints.

use std::collections::HashSet;

use axum::extract::{Multipart, Query, State};
use axum::response::{IntoResponse, Json};
use axum::http::StatusCode;
use pdf_inspector::{MarkdownOptions, MarkdownProfile, PdfOptions, ProcessMode};
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, warn};

use crate::dto::{ConvertResponse, DetectResponse, ErrorResponse, PdfUploadBody};
use crate::error::ApiError;
use crate::state::AppState;

/// Query parameters for the conversion endpoint.
#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ConvertQuery {
    /// Pages to extract, 1-indexed, e.g. `1,3,5-10`. Omit for all pages.
    pub select_pages: Option<String>,
    /// Password for an encrypted PDF.
    pub password: Option<String>,
    /// `true` to collapse token-heavy source formatting (dot leaders, etc.).
    pub compact: Option<bool>,
}

/// Query parameters for the detect endpoint.
#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DetectQuery {
    /// Password for an encrypted PDF.
    pub password: Option<String>,
}

/// `POST /convert` — classify the PDF and extract Markdown.
#[utoipa::path(
    post,
    path = "/convert",
    tag = "pdf",
    request_body(content = PdfUploadBody, content_type = "multipart/form-data", description = "PDF file uploaded as multipart field `file`"),
    params(ConvertQuery),
    responses(
        (status = 200, description = "PDF processed; Markdown returned in `markdown` (null when OCR is needed).", body = ConvertResponse),
        (status = 400, description = "`missing_file`, `empty_upload`, or `not_a_pdf`.", body = ErrorResponse),
        (status = 413, description = "Upload exceeds `MAX_BODY_MB`.", body = ErrorResponse),
        (status = 422, description = "`invalid_structure`, `parse_error`, or `encrypted` (supply `password`).", body = ErrorResponse),
        (status = 500, description = "`internal_error`.", body = ErrorResponse),
        (status = 503, description = "`server_busy` — at capacity, retry later.", body = ErrorResponse),
    )
)]
pub async fn convert(
    State(state): State<AppState>,
    Query(query): Query<ConvertQuery>,
    mut multipart: Multipart,
) -> Result<Json<ConvertResponse>, ApiError> {
    let bytes = read_pdf_field(&mut multipart).await?;
    let options = build_options(ProcessMode::Full, &query.select_pages, &query.password, query.compact);

    let result = process_in_slot(&state, bytes, options).await?;
    debug!(pdf_type = ?result.pdf_type, pages = result.page_count, "convert ok");
    Ok(Json(ConvertResponse::from(result)))
}

/// `POST /detect` — fast classification only (no extraction / markdown).
#[utoipa::path(
    post,
    path = "/detect",
    tag = "pdf",
    request_body(content = PdfUploadBody, content_type = "multipart/form-data", description = "PDF file uploaded as multipart field `file`"),
    params(DetectQuery),
    responses(
        (status = 200, description = "Classification result (no Markdown).", body = DetectResponse),
        (status = 400, description = "`missing_file`, `empty_upload`, or `not_a_pdf`.", body = ErrorResponse),
        (status = 413, description = "Upload exceeds `MAX_BODY_MB`.", body = ErrorResponse),
        (status = 422, description = "`invalid_structure`, `parse_error`, or `encrypted` (supply `password`).", body = ErrorResponse),
        (status = 500, description = "`internal_error`.", body = ErrorResponse),
        (status = 503, description = "`server_busy` — at capacity, retry later.", body = ErrorResponse),
    )
)]
pub async fn detect(
    State(state): State<AppState>,
    Query(query): Query<DetectQuery>,
    mut multipart: Multipart,
) -> Result<Json<DetectResponse>, ApiError> {
    let bytes = read_pdf_field(&mut multipart).await?;
    let options = build_options(ProcessMode::DetectOnly, &None, &query.password, None);

    let result = process_in_slot(&state, bytes, options).await?;
    debug!(pdf_type = ?result.pdf_type, pages = result.page_count, "detect ok");
    Ok(Json(DetectResponse::from(result)))
}

/// `GET /health` — liveness/readiness probe for orchestrators.
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, description = "Service is up.", content_type = "application/json",
         example = json!({"status": "ok"})),
    )
)]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

// --- helpers ---------------------------------------------------------------

/// Pull the first PDF file from a multipart form into memory.
async fn read_pdf_field(multipart: &mut Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::Internal(format!("multipart read error: {e}")))?
    {
        // Accept any field whose name is `file` or that carries a filename;
        // callers using `curl -F file=@doc.pdf` hit both.
        let has_filename = field.file_name().is_some();
        if field.name() == Some("file") || has_filename {
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::Internal(format!("field read error: {e}")))?;
            if data.is_empty() {
                return Err(ApiError::EmptyUpload);
            }
            return Ok(data.to_vec());
        }
    }
    Err(ApiError::MissingFile)
}

/// Translate the query string into `PdfOptions`.
fn build_options(
    mode: ProcessMode,
    select_pages: &Option<String>,
    password: &Option<String>,
    compact: Option<bool>,
) -> PdfOptions {
    let mut options = PdfOptions::new().mode(mode);

    if let Some(spec) = select_pages {
        let pages = parse_page_spec(spec);
        if !pages.is_empty() {
            options = options.pages(pages);
        }
    }
    if let Some(pw) = password {
        if !pw.is_empty() {
            options = options.password(pw);
        }
    }
    if compact == Some(true) {
        options.markdown.profile = MarkdownProfile::Compact;
    } else {
        options.markdown = MarkdownOptions::default();
    }
    options
}

/// Parse a page selector like `1,3,5-10` into a sorted, deduped set.
fn parse_page_spec(spec: &str) -> Vec<u32> {
    let mut pages: HashSet<u32> = HashSet::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start, end)) = part.split_once('-') {
            if let (Ok(start), Ok(end)) = (start.trim().parse::<u32>(), end.trim().parse::<u32>()) {
                if start <= end {
                    pages.extend(start..=end);
                }
            }
            continue;
        }
        if let Ok(n) = part.parse::<u32>() {
            pages.insert(n);
        }
    }
    let mut v: Vec<u32> = pages.into_iter().collect();
    v.sort_unstable();
    v
}

/// Acquire a processing slot, then run the CPU-heavy library call on the
/// blocking pool so the async runtime stays free to serve other requests.
async fn process_in_slot(
    state: &AppState,
    bytes: Vec<u8>,
    options: PdfOptions,
) -> Result<pdf_inspector::PdfProcessResult, ApiError> {
    // Wait for a free slot, bounded by a queue timeout so an overloaded server
    // fails fast (503) instead of piling up unbounded waiters.
    let permit = match tokio::time::timeout(
        state.config.queue_timeout,
        state.processing_slots.acquire(),
    )
    .await
    {
        Ok(Ok(permit)) => permit,
        // A closed semaphore only happens at shutdown; treat as busy.
        Ok(Err(_)) => return Err(ApiError::ServerBusy),
        Err(_) => return Err(ApiError::ServerBusy),
    };

    // The library is synchronous and CPU-bound (lopdf + rayon internally).
    // Run it off the async executor and surface panics as 500s.
    let result = tokio::task::spawn_blocking(move || {
        pdf_inspector::process_pdf_mem_with_options(&bytes, options)
    })
    .await;

    drop(permit);

    match result {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(ApiError::from(e)),
        Err(join_err) => {
            warn!(error = %join_err, "processing task panicked");
            Err(ApiError::Internal(format!("processing failed: {join_err}")))
        }
    }
}

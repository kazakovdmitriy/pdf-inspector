//! Error type and HTTP status mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use pdf_inspector::PdfError;
use serde_json::json;

/// Errors produced by the API handlers.
#[derive(Debug)]
pub enum ApiError {
    /// No `file` part in the multipart form, or it had no filename.
    MissingFile,
    /// The upload exists but is empty (0 bytes).
    EmptyUpload,
    /// The supplied bytes are not a PDF (bad magic / header).
    NotAPdf(String),
    /// A valid PDF whose structure could not be parsed.
    InvalidStructure,
    /// A parse failure with a free-form message from the library.
    Parse(String),
    /// PDF requires a password; none was supplied or it was wrong.
    Encrypted,
    /// All processing slots are busy and the queue timeout elapsed.
    ServerBusy,
    /// A panic or internal failure in the processing thread.
    Internal(String),
}

impl From<PdfError> for ApiError {
    fn from(e: PdfError) -> Self {
        match e {
            PdfError::Io(e) => ApiError::Parse(format!("IO error: {e}")),
            PdfError::NotAPdf(msg) => ApiError::NotAPdf(msg),
            PdfError::InvalidStructure => ApiError::InvalidStructure,
            PdfError::Encrypted => ApiError::Encrypted,
            PdfError::Parse(msg) => ApiError::Parse(msg),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::MissingFile => (
                StatusCode::BAD_REQUEST,
                "missing_file",
                "No 'file' field found in multipart form. Expected a PDF upload."
                    .to_string(),
            ),
            ApiError::EmptyUpload => (
                StatusCode::BAD_REQUEST,
                "empty_upload",
                "The uploaded file is empty (0 bytes).".to_string(),
            ),
            ApiError::NotAPdf(msg) => (
                StatusCode::BAD_REQUEST,
                "not_a_pdf",
                format!("The uploaded file is not a valid PDF: {msg}"),
            ),
            ApiError::InvalidStructure => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_structure",
                "The PDF structure could not be parsed (corrupt or unsupported)."
                    .to_string(),
            ),
            ApiError::Parse(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "parse_error",
                msg,
            ),
            ApiError::Encrypted => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "encrypted",
                "PDF is encrypted. Supply the password via the `password` query parameter."
                    .to_string(),
            ),
            ApiError::ServerBusy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "server_busy",
                "Server is at capacity. Retry later or raise MAX_CONCURRENT.".to_string(),
            ),
            ApiError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg,
            ),
        };

        let body = Json(json!({
            "error": code,
            "message": message,
        }));
        (status, body).into_response()
    }
}

//! JSON response models.
//!
//! The library's own result types (`PdfProcessResult`, `PdfType`, ...) don't
//! implement `serde::Serialize`, so we mirror them here into serializable DTOs
//! and convert via `From`. This keeps the library untouched while controlling
//! the exact wire format. Field names match the CLI's `--json` output and the
//! Python/Node bindings for cross-language consistency.

use pdf_inspector::{LayoutComplexity, PageOcrReasons, PdfProcessResult, PdfType};
use serde::Serialize;

/// One row of `pages_needing_ocr` context: which page and why OCR is advised.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageOcrReasonsDto {
    /// 1-indexed page number.
    pub page: u32,
    /// Machine-readable reason identifiers (e.g. "scanned", "no_text").
    pub reasons: Vec<String>,
}

/// Layout complexity summary.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LayoutComplexityDto {
    pub is_complex: bool,
    /// 1-indexed pages containing detected tables.
    pub pages_with_tables: Vec<u32>,
    /// 1-indexed pages with multi-column text.
    pub pages_with_columns: Vec<u32>,
}

impl From<LayoutComplexity> for LayoutComplexityDto {
    fn from(l: LayoutComplexity) -> Self {
        Self {
            is_complex: l.is_complex,
            pages_with_tables: l.pages_with_tables,
            pages_with_columns: l.pages_with_columns,
        }
    }
}

impl From<PageOcrReasons> for PageOcrReasonsDto {
    fn from(r: PageOcrReasons) -> Self {
        Self {
            page: r.page,
            reasons: r.reasons,
        }
    }
}

/// Serialize `PdfType` as a lowercase snake_case string, matching the CLI and
/// the Python binding ("text_based", "scanned", "image_based", "mixed").
pub fn pdf_type_str(t: PdfType) -> &'static str {
    match t {
        PdfType::TextBased => "text_based",
        PdfType::Scanned => "scanned",
        PdfType::ImageBased => "image_based",
        PdfType::Mixed => "mixed",
    }
}

/// Response for `POST /convert`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConvertResponse {
    pub pdf_type: &'static str,
    /// Extracted Markdown. `None` when the PDF needs OCR (scanned/image-based).
    pub markdown: Option<String>,
    pub page_count: u32,
    pub processing_time_ms: u64,
    pub confidence: f32,
    pub title: Option<String>,
    pub has_encoding_issues: bool,
    /// 1-indexed pages that should be routed to OCR.
    pub pages_needing_ocr: Vec<u32>,
    pub ocr_reasons_by_page: Vec<PageOcrReasonsDto>,
    pub layout: LayoutComplexityDto,
}

/// Response for `POST /detect` — classification only, no markdown.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DetectResponse {
    pub pdf_type: &'static str,
    pub page_count: u32,
    pub processing_time_ms: u64,
    pub confidence: f32,
    pub title: Option<String>,
    pub has_encoding_issues: bool,
    /// 1-indexed pages that should be routed to OCR.
    pub pages_needing_ocr: Vec<u32>,
    pub ocr_reasons_by_page: Vec<PageOcrReasonsDto>,
    pub layout: LayoutComplexityDto,
}

impl From<PdfProcessResult> for ConvertResponse {
    fn from(r: PdfProcessResult) -> Self {
        Self {
            pdf_type: pdf_type_str(r.pdf_type),
            markdown: r.markdown,
            page_count: r.page_count,
            processing_time_ms: r.processing_time_ms,
            confidence: r.confidence,
            title: r.title,
            has_encoding_issues: r.has_encoding_issues,
            pages_needing_ocr: r.pages_needing_ocr,
            ocr_reasons_by_page: r.ocr_reasons_by_page.into_iter().map(Into::into).collect(),
            layout: r.layout.into(),
        }
    }
}

impl From<PdfProcessResult> for DetectResponse {
    fn from(r: PdfProcessResult) -> Self {
        // detect_only() never produces markdown, so the field is intentionally
        // absent from the response shape.
        Self {
            pdf_type: pdf_type_str(r.pdf_type),
            page_count: r.page_count,
            processing_time_ms: r.processing_time_ms,
            confidence: r.confidence,
            title: r.title,
            has_encoding_issues: r.has_encoding_issues,
            pages_needing_ocr: r.pages_needing_ocr,
            ocr_reasons_by_page: r.ocr_reasons_by_page.into_iter().map(Into::into).collect(),
            layout: r.layout.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Types used only for OpenAPI documentation (not by the request handlers).
// ---------------------------------------------------------------------------

/// JSON error body returned on non-2xx responses.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    /// Machine-readable error code, e.g. `missing_file`, `encrypted`.
    pub error: String,
    /// Human-readable explanation of the failure.
    pub message: String,
}

/// Documentation-only shape for the multipart upload. The real handlers read
/// the request with `axum::extract::Multipart`; this struct exists purely so
/// the OpenAPI spec can describe the expected form field.
#[derive(Debug, utoipa::ToSchema)]
pub struct PdfUploadBody {
    /// The PDF file. The field name must be `file`
    /// (e.g. `curl -F file=@document.pdf`).
    #[allow(dead_code)]
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

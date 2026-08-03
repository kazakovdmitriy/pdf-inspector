//! OpenAPI specification assembled from `#[utoipa::path]` annotations.

use utoipa::OpenApi;

use crate::dto::{ConvertResponse, DetectResponse, ErrorResponse, LayoutComplexityDto, PageOcrReasonsDto, PdfUploadBody};
use crate::handlers;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "pdf-inspector API",
        version = "0.1.0",
        description = "Fast PDF classification and Markdown extraction. Detects whether a PDF is \
                       text-based or scanned and converts text-based PDFs to clean Markdown without OCR.",
        license(name = "MIT"),
    ),
    paths(
        handlers::convert,
        handlers::detect,
        handlers::health,
    ),
    components(
        schemas(
            ConvertResponse,
            DetectResponse,
            ErrorResponse,
            PdfUploadBody,
            PageOcrReasonsDto,
            LayoutComplexityDto,
        )
    ),
    tags(
        (name = "pdf", description = "PDF classification and extraction"),
        (name = "system", description = "Service health"),
    )
)]
pub struct ApiDoc;

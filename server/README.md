# pdf-inspector-api

HTTP REST API wrapping the [`pdf-inspector`](..) Rust library. Classify PDFs
(text-based vs. scanned) and extract clean Markdown over a small JSON service —
useful when you want to deploy the parser once and call it from any language.

Single self-contained binary, pure Rust, no external services.

## Endpoints

| Method | Path      | Description                                              |
|--------|-----------|----------------------------------------------------------|
| `POST` | `/convert`| Classify the PDF **and** extract Markdown.              |
| `POST` | `/detect` | Fast classification only (~1–50 ms), no extraction.     |
| `GET`  | `/health` | Liveness probe → `{"status":"ok"}`.                      |

### Query parameters

| Endpoint   | Param          | Type    | Description                                              |
|------------|----------------|---------|----------------------------------------------------------|
| `/convert` | `select_pages` | string  | 1-indexed pages to extract, e.g. `1,3,5-10`. Optional.  |
| `/convert` | `password`     | string  | Password for an encrypted PDF. Optional.                 |
| `/convert` | `compact`      | bool    | Collapse token-heavy formatting (dot leaders). Optional. |
| `/detect`  | `password`     | string  | Password for an encrypted PDF. Optional.                 |

The PDF is sent as a multipart form field named `file`.

## Interactive API docs

An interactive Scalar API reference is served at **`GET /docs`** (OpenAPI 3.1):
browse endpoints, inspect request/response schemas, and send live test requests
from the browser.

## Run locally

```bash
cargo run --release
# listening on 0.0.0.0:3000
```

## Run with Docker

```bash
# From the repository root (context needs the library + server crates):
docker build -f server/Dockerfile -t pdf-inspector-api .

docker run --rm -p 3000:3000 pdf-inspector-api
```

Or with docker compose:

```bash
docker compose -f server/docker-compose.yml up --build
```

## Examples

### Detect the type of a PDF

```bash
curl -X POST http://localhost:3000/detect \
  -F "file=@document.pdf"
```

```json
{
  "pdf_type": "text_based",
  "page_count": 10,
  "processing_time_ms": 1,
  "confidence": 1.0,
  "title": "Publication 1244 (Rev. July 1996)",
  "has_encoding_issues": false,
  "pages_needing_ocr": [],
  "ocr_reasons_by_page": [],
  "layout": {
    "is_complex": false,
    "pages_with_tables": [],
    "pages_with_columns": []
  }
}
```

`pdf_type` is one of `text_based`, `scanned`, `image_based`, `mixed`. When it is
`scanned` or `image_based`, the document needs OCR and `pages_needing_ocr`
lists the 1-indexed pages to send through your OCR pipeline.

### Convert to Markdown

```bash
curl -X POST "http://localhost:3000/convert?compact=true" \
  -F "file=@document.pdf"
```

Same shape as `/detect` but with an added `markdown` field (a string, or `null`
when the PDF needs OCR):

```json
{
  "pdf_type": "text_based",
  "markdown": "# Title\n\nBody text…",
  "page_count": 10,
  "processing_time_ms": 5,
  ...
}
```

### Extract specific pages

```bash
curl -X POST "http://localhost:3000/convert?select_pages=1,3,5-10" \
  -F "file=@document.pdf"
```

### Encrypted PDF

```bash
curl -X POST "http://localhost:3000/detect?password=secret123" \
  -F "file=@encrypted.pdf"
```

## Error responses

Errors return a JSON body `{"error": "<code>", "message": "…"}` with an
appropriate HTTP status:

| HTTP | `error`            | When                                                |
|------|--------------------|-----------------------------------------------------|
| 400  | `missing_file`     | No `file` field in the multipart form.              |
| 400  | `empty_upload`     | The uploaded file is empty.                         |
| 400  | `not_a_pdf`        | Bytes are not a valid PDF (bad header).             |
| 413  | —                  | Request body exceeds `MAX_BODY_MB`.                 |
| 422  | `encrypted`        | PDF is encrypted; supply `password`.                |
| 422  | `invalid_structure`| Corrupt or unsupported PDF structure.               |
| 422  | `parse_error`      | A parsing failure with a library message.           |
| 503  | `server_busy`      | At capacity; the queue timeout elapsed.             |
| 500  | `internal_error`   | Unexpected failure during processing.               |

## Configuration (environment variables)

| Variable             | Default | Description                                              |
|----------------------|---------|----------------------------------------------------------|
| `PORT`               | `3000`  | TCP port to listen on.                                   |
| `MAX_BODY_MB`        | `100`   | Maximum upload size in MiB.                              |
| `MAX_CONCURRENT`     | CPUs    | Max PDFs processed at once. `0` = auto (number of CPUs). |
| `QUEUE_TIMEOUT_SECS` | `60`    | How long a request waits for a free processing slot.     |
| `RUST_LOG`           | `info`  | Log filter (e.g. `debug`, `info,pdf_inspector_api=debug`).|

## Performance notes

The library is CPU-bound and uses rayon internally. Each request acquires one
of `MAX_CONCURRENT` processing slots and runs on the blocking pool so the async
runtime keeps serving other requests. If all slots are busy, new requests wait
up to `QUEUE_TIMEOUT_SECS` and then return `503 server_busy` rather than
queuing unboundedly.

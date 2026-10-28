use std::sync::LazyLock;

use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::{IntoResponse, Response},
};

use prometheus::{
    register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};

static HTTP_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "chainflow_http_requests_total",
        "Total HTTP requests",
        &["method", "path", "status"]
    )
    .expect("register request counter")
});

static HTTP_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "chainflow_http_request_duration_seconds",
        "HTTP request duration",
        &["method", "path"]
    )
    .expect("register duration histogram")
});

pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().to_string();

    // Use the matched route template, not raw URL paths.
    let path = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string());

    let timer = HTTP_DURATION
        .with_label_values(&[&method, &path])
        .start_timer();

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();

    HTTP_REQUESTS
        .with_label_values(&[&method, &path, &status])
        .inc();

    timer.observe_duration();

    response
}

pub async fn metrics_handler() -> impl IntoResponse {
    // Force registration even before the first request.
    LazyLock::force(&HTTP_REQUESTS);
    LazyLock::force(&HTTP_DURATION);

    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buffer = Vec::new();

    match encoder.encode(&families, &mut buffer) {
        Ok(()) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
            )],
            String::from_utf8_lossy(&buffer).into_owned(),
        )
            .into_response(),
        Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

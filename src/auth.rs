use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

use subtle::ConstantTimeEq;

pub async fn require_api_key(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = std::env::var("CHAINFLOW_API_KEY")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let provided = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let valid = provided.as_bytes().ct_eq(expected.as_bytes());

    if !bool::from(valid) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
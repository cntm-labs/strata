use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    status: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Request(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        // Capture only operational errors. 4xx variants are user errors and
        // would explode Sentry quota for no diagnostic benefit. NOTE: the
        // captured message inherits the `e.to_string()` payload from sqlx /
        // reqwest — defensively avoid binding tenant content into SQL via
        // format!() so this can't leak per-row data.
        if status.is_server_error() {
            sentry::capture_error(&self);
        }

        let body = ErrorResponse {
            code: status.as_u16(),
            status: status.canonical_reason().unwrap_or("Error").to_string(),
            message,
        };

        (status, axum::Json(body)).into_response()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;

    async fn error_to_parts(error: AppError) -> (StatusCode, ErrorResponse) {
        let response = error.into_response();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let err_body: ErrorResponse = serde_json::from_slice(&body).unwrap();
        (status, err_body)
    }

    #[derive(serde::Deserialize)]
    struct ErrorResponse {
        code: u16,
        status: String,
        message: String,
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let (status, body) = error_to_parts(AppError::NotFound("missing".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.code, 404);
        assert_eq!(body.status, "Not Found");
        assert_eq!(body.message, "missing");
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let (status, body) = error_to_parts(AppError::BadRequest("invalid".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, 400);
        assert_eq!(body.status, "Bad Request");
        assert_eq!(body.message, "invalid");
    }

    #[tokio::test]
    async fn database_error_returns_500() {
        let db_err = sqlx::Error::RowNotFound;
        let (status, body) = error_to_parts(AppError::Database(db_err)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, 500);
    }

    #[tokio::test]
    async fn internal_error_returns_500() {
        let (status, body) = error_to_parts(AppError::Internal("oops".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, 500);
        assert_eq!(body.message, "oops");
    }

    #[tokio::test]
    async fn request_error_returns_502() {
        let req_err = reqwest::get("http://127.0.0.1:1/nonexistent")
            .await
            .unwrap_err();
        let (status, body) = error_to_parts(AppError::Request(req_err)).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body.code, 502);
    }

    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let app_err = AppError::from(json_err);
        assert!(matches!(app_err, AppError::Internal(_)));
    }

    #[test]
    fn display_messages() {
        assert_eq!(AppError::NotFound("x".into()).to_string(), "Not found: x");
        assert_eq!(
            AppError::BadRequest("y".into()).to_string(),
            "Bad request: y"
        );
        assert_eq!(
            AppError::Internal("z".into()).to_string(),
            "Internal error: z"
        );
    }

    #[test]
    fn server_error_captures_sentry_event() {
        let events = sentry::test::with_captured_events(|| {
            let _ = AppError::Internal("boom".to_string()).into_response();
        });
        assert_eq!(events.len(), 1, "expected exactly one captured event");
    }

    #[test]
    fn database_error_captures_sentry_event() {
        let events = sentry::test::with_captured_events(|| {
            let _ = AppError::Database(sqlx::Error::PoolTimedOut).into_response();
        });
        assert_eq!(events.len(), 1, "expected exactly one captured event");
    }

    #[test]
    fn client_error_skips_sentry() {
        let events = sentry::test::with_captured_events(|| {
            let _ = AppError::NotFound("widget".to_string()).into_response();
            let _ = AppError::BadRequest("malformed".to_string()).into_response();
        });
        assert_eq!(
            events.len(),
            0,
            "client errors must not be captured; got {events:?}"
        );
    }

    #[test]
    fn app_result_type_alias_works() {
        let ok: AppResult<i32> = Ok(42);
        assert!(ok.is_ok());
        let err: AppResult<i32> = Err(AppError::NotFound("nope".into()));
        assert!(err.is_err());
    }
}

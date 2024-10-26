use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    UnprocessableEntiry(String),
    #[error("{0}")]
    EntityNotFound(String),
    #[error("{0}")]
    ValidationError(#[from] garde::Report),
    // sqlx::Errorを引数にするヴァリアントが複数あるので、[from]は使えず、[source]で代用している
    #[error(" トランザクションを実行できませんでした。")]
    TransactionError(#[source] sqlx::Error),
    #[error(" データベース処理実行中にエラーが発生しました。")]
    SpecificOperationError(#[source] sqlx::Error),
    #[error("Not rows affected: {0}")]
    NoRowAffectedError(String),
    #[error("{0}")]
    KeyValueStoreError(#[from] redis::RedisError),
    #[error("{0}")]
    BcriptError(#[from] bcrypt::BcryptError),
    #[error("{0}")]
    ConvertToUuidError(#[from] uuid::Error),
    #[error("ログインに失敗しました")]
    UnauthenticatedError,
    #[error("認可情報が間違っています")]
    UnauthorizedError,
    #[error("許可されていない操作です")]
    ForbiddenOperation,
    #[error("{0}")]
    ConversionEntityError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self {
            AppError::UnprocessableEntiry(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::EntityNotFound(_) => StatusCode::NOT_FOUND,
            AppError::ValidationError(_) | AppError::ConvertToUuidError(_) => {
                StatusCode::BAD_REQUEST
            }
            AppError::UnauthenticatedError | AppError::ForbiddenOperation => StatusCode::FORBIDDEN,
            AppError::UnauthorizedError => StatusCode::UNAUTHORIZED,
            e @ (AppError::TransactionError(_)
            | AppError::SpecificOperationError(_)
            | AppError::NoRowAffectedError(_)
            | AppError::KeyValueStoreError(_)
            | AppError::BcriptError(_)
            | AppError::ConversionEntityError(_)) => {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Unexpected error happened"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        status_code.into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts, RequestPartsExt};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use kernel::model::{auth::AccessToken, id::UserId, role::Role, user::User};
use registry::AppRegistry;
use shared::error::AppError;

// リクエストの前処理を実行後、handler に渡す構造体を定義
pub struct AuthorizedUser {
    pub access_token: AccessToken,
    pub user: User,
}

impl AuthorizedUser {
    pub fn id(&self) -> UserId {
        self.user.id
    }

    pub fn is_admin(&self) -> bool {
        self.user.role == Role::Admin
    }
}

#[async_trait]
impl FromRequestParts<AppRegistry> for AuthorizedUser {
    type Rejection = AppError;

    // handler メソッドの引数に AuthorizedUser を追加したときはこのメソッドが呼ばれる
    async fn from_request_parts(
        parts: &mut Parts,
        registry: &AppRegistry,
    ) -> Result<Self, Self::Rejection> {
        // HTTP ヘッダからアクセストークンを取り出す
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::UnauthorizedError)?;
        let access_token = AccessToken(bearer.token().to_string());

        // アクセストークンが紐づくユーザ ID を抽出
        let user_id = registry
            .auth_repository()
            .fetch_user_id_from_token(&access_token)
            .await?
            .ok_or(AppError::UnauthenticatedError)?;

        // ユーザ ID でデータベースからユーザのレコードを引く
        let user = registry
            .user_repository()
            .find_current_user(user_id)
            .await?
            .ok_or(AppError::UnauthenticatedError)?;

        Ok(Self { access_token, user })
    }
}

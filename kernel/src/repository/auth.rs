use shared::error::AppResult;

use crate::model::{auth::Accesstoken, id::UserId};

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn fetch_user_id_from_token(
        &self,
        access_token: &Accesstoken,
    ) -> AppResult<Option<UserId>>;

    async fn verify_user(&self, email: &str, password: &str) -> AppResult<UserId>;

    async fn create_token(&self, event: CreateToken) -> AppResult<Accesstoken>;

    async fn delete_token(&self, access_token: &Accesstoken) -> AppResult<()>;
}

use async_trait::async_trait;
use kernel::{
    model::{auth::AccessToken, id::UserId},
    repository::auth::AuthRepository,
};
use shared::error::AppResult;

use crate::{
    database::{
        model::auth::{AuthorizationKey, AuthorizedUserId},
        ConnectionPool,
    },
    redis::RedisClient,
};

#[derive(new)]
pub struct AuthRepositoryImpl {
    db: ConnectionPool,
    kv: Arc<RedisClient>,
    ttl: u64,
}

#[async_trait]
impl AuthRepository for AuthRepositoryImpl {
    async fn fetch_user_id_from_token(
        &self,
        access_token: &AccessToken,
    ) -> AppResult<Option<UserId>> {
        let key: AuthorizationKey = access_token.into();
        self.kv
            .get(&key)
            .await
            .map(|x| x.map(AuthorizedUserId::into_inner))
    }
}

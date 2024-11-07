use axum::routing::{delete, get, put};
use axum::Router;
use registry::AppRegistry;

use crate::handler::user::{
    change_password, change_role, delete_user, get_checkouts, get_current_user, list_users,
    register_user,
};

// me がパスに入っているリクエストはリクエストを送る自分自身しかできないという設計
// 実際にそうなってるか確認したいかも
// 見る感じしてなさそうだけど？
pub fn build_user_router() -> Router<AppRegistry> {
    Router::new()
        .route("/users/me", get(get_current_user))
        .route("/users/me/password", put(change_password))
        .route("/users/me/checkouts", get(get_checkouts))
        .route("/users", get(list_users).post(register_user))
        .route("/users/:user_id", delete(delete_user))
        .route("/users/:user_id/role", put(change_role))
}

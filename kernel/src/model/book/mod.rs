use chrono::{DateTime, Utc};

use super::{
    id::{BookId, CheckoutId},
    user::{BookOwner, CheckoutUser},
};

pub mod event;

#[derive(Debug)]
pub struct Book {
    pub id: BookId,
    pub title: String,
    pub author: String,
    pub isbn: String,
    pub description: String,
    pub owner: BookOwner,
    pub checkout: Option<Checkout>,
}

// ページネーションの範囲を指定するための設定値を格納する型を追加
#[derive(Debug)]
pub struct BookListOptions {
    pub limit: i64,
    pub offset: i64,
}

// この型は model::checkout モジュール側でも同名の型を定義しているが、
// それと異なるモジュールにあるので別の型として扱われる。
// 実際、上記 `Book` 型の checkout フィールドとしてのみ使用する
#[derive(Debug)]
pub struct Checkout {
    pub checkout_id: CheckoutId,
    pub checked_out_by: CheckoutUser,
    pub checked_out_at: DateTime<Utc>,
}

use async_trait::async_trait;
use derive_new::new;
use kernel::{
    model::{
        checkout::{
            event::{CreateCheckout, UpdateReturned},
            Checkout,
        },
        id::{BookId, CheckoutId, UserId},
    },
    repository::checkout::CheckoutRepository,
};
use shared::error::{AppError, AppResult};

use crate::database::{
    model::checkout::{CheckoutRow, CheckoutStateRow, ReturnedCheckoutRow},
    ConnectionPool,
};

#[derive(new)]
pub struct CheckoutRepositoryImpl {
    db: ConnectionPool,
}

#[async_trait]
impl CheckoutRepository for CheckoutRepositoryImpl {
    // 貸し出し操作
    async fn create(&self, event: CreateCheckout) -> AppResult<()> {
        let mut tx = self.db.begin().await?;

        // トランザクション分離レベルを SERIALIZABLE に設定する
        self.set_transaction_serializable(&mut tx).await?;

        // 事前のチェックとして以下を調べる
        // - 指定の蔵書の ID を持つ蔵書が存在するか
        // - 存在した場合、この蔵書は貸し出し中でないか
        //
        // 上記の両方が Yes だった場合、このブロック以降の処理に進む
        {
            let res = sqlx::query_as!(
                CheckoutStateRow,
                r#"
                    SELECT
                    b.book_id,
                    c.checkout_id AS "checkout_id?: CheckoutId",
                    NULL AS "user_id?: UserId"
                    FROM books AS b
                    LEFT OUTER JOIN checkouts AS c USING(book_id)
                    WHERE book_id = $1;
                "#,
                event.book_id as _
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::SpecificOperationError)?;

            match res {
                // 指定した書籍が存在しない場合
                None => {
                    return Err(AppError::EntityNotFound(format!(
                        "書籍（{}）が見つかりませんでした。",
                        event.book_id
                    )))
                }
                Some(CheckoutStateRow {
                    checkout_id: Some(_),
                    ..
                }) => {
                    return Err(AppError::UnprocessableEntiry(format!(
                        "書籍（{}）に対する貸出が既に存在します。",
                        event.book_id
                    )))
                }
                _ => {}
            }
        }

        // 貸し出し処理を行う
        let checkout_id = CheckoutId::new();
        let res = sqlx::query!(
            r#"
                INSERT INTO checkouts
                (checkout_id, book_id, user_id, checked_out_at)
                VALUES ($1, $2, $3, $4);
            "#,
            checkout_id as _,
            event.book_id as _,
            event.checked_out_by as _,
            event.checked_out_at,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::SpecificOperationError)?;

        if res.rows_affected() < 1 {
            return Err(AppError::NoRowAffectedError(
                "No checkout record has been created".into(),
            ));
        }

        tx.commit().await.map_err(AppError::TransactionError)?;

        Ok(())
    }

    // 返却処理を行う
    // Q. ここの event.checkout_id はどこから来てるか調べよう
    async fn update_returned(&self, event: UpdateReturned) -> AppResult<()> {
        let mut tx = self.db.begin().await?;

        self.set_transaction_serializable(&mut tx).await?;

        // 返却操作時は事前のチェックとして、以下を調べる
        // - 指定の蔵書 ID を持つ蔵書が存在するか
        // - 存在した場合
        //   - この蔵書は貸し出し中であり
        //   - かつ借りたユーザーが指定のユーザーであるか
        // 上記の両方が Yes だった場合、このブロック以降の処理に進む
        {
            let res = sqlx::query_as!(
                CheckoutStateRow,
                r#"
                    SELECT
                    b.book_id,
                    c.checkout_id AS "checkout_id?: CheckoutId",
                    c.user_id AS "user_id?: UserId"
                    FROM books AS b
                    LEFT OUTER JOIN checkouts AS c USING(book_id)
                    WHERE book_id = $1;
                "#,
                event.book_id as _,
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(AppError::SpecificOperationError)?;

            match res {
                None => {
                    return Err(AppError::EntityNotFound(format!(
                        "書籍（{}）が見つかりませんでした。",
                        event.book_id
                    )))
                }
                Some(CheckoutStateRow {
                    checkout_id: Some(c),
                    user_id: Some(u),
                    ..
                }) if (c, u) != (event.checkout_id, event.returned_by) => {
                    return Err(AppError::UnprocessableEntiry(format!(
                        "指定の貸出（ID（{}）、ユーザー（{}）、書籍（{}））は返却できません。",
                        event.checkout_id, event.returned_by, event.book_id
                    )))
                }
                // あれ、checkout_id とかが None の場合とかはここでは検査しない？
                // うまく表現できないけど、検査はしていないように見えるけど、
                // 実質的に見ないといけないのが上の条件だけみたいなパターンがありそう
                _ => {}
            }
        }

        let res = sqlx::query!(
            r#"
                INSERT INTO returned_checkouts
                (checkout_id, book_id, user_id, checked_out_at, returned_at)
                SELECT checkout_id, book_id, user_id, checked_out_at, $2
                FROM checkouts
                WHERE checkout_id = $1;
            "#,
            event.checkout_id as _,
            event.returned_at,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::SpecificOperationError)?;

        if res.rows_affected() < 1 {
            return Err(AppError::NoRowAffectedError(
                "No returning record has been update".into(),
            ));
        }

        // 上記処理が成功したら checkouts テーブルから該当貸出 ID のレコードを削除する
        let res = sqlx::query!(
            r#"
                DELETE FROM checkouts WHERE checkout_id = $1;
            "#,
            event.checkout_id as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::SpecificOperationError)?;

        if res.rows_affected() < 1 {
            return Err(AppError::NoRowAffectedError(
                "No checkout record has been deleted".into(),
            ));
        }

        tx.commit().await.map_err(AppError::TransactionError)?;

        Ok(())
    }

    async fn find_unreturned_all(&self) -> AppResult<Vec<Checkout>> {
        // checkouts テーブルにあるレコードを全権抽出する
        // books テーブルと INNER JOIN して、蔵書の情報も一緒に抽出する
        // 出力するレコードは貸出日の古い順に並べる
        sqlx::query_as!(
            CheckoutRow,
            r#"
                SELECT
                c.checkout_id,
                c.book_id,
                c.user_id,
                c.checked_out_at,
                b.title,
                b.author,
                b.isbn
                FROM checkouts AS c
                INNER JOIN books AS b USING(book_id)
                ORDER BY c.checked_out_at ASC;
            "#,
        )
        .fetch_all(self.db.inner_ref())
        .await
        .map(|rows| rows.into_iter().map(Checkout::from).collect())
        .map_err(AppError::SpecificOperationError)
    }

    // ユーザー　ID に紐づく未返却の貸出情報を取得する
    async fn find_unreturned_by_user_id(&self, user_id: UserId) -> AppResult<Vec<Checkout>> {
        // find_unreturned_all の SQL に
        // ユーザー ID で絞り込む WHERE を追加したもの
        sqlx::query_as!(
            CheckoutRow,
            r#"
                SELECT
                c.checkout_id,
                c.book_id,
                c.user_id,
                c.checked_out_at,
                b.title,
                b.author,
                b.isbn
                FROM checkouts AS c
                INNER JOIN books AS b USING(book_id)
                WHERE c.user_id = $1
                ORDER BY c.checked_out_at ASC;
            "#,
            user_id as _
        )
        .fetch_all(self.db.inner_ref())
        .await
        .map(|rows| rows.into_iter().map(Checkout::from).collect())
        .map_err(AppError::SpecificOperationError)
    }

    // 蔵書の貸出履歴（返却済みも含む）を取得する
    async fn find_history_by_book_id(&self, book_id: BookId) -> AppResult<Vec<Checkout>> {
        // このメソッドは貸出中・返却済みの両方を取得して
        // 蔵書に対する貸出履歴の一覧として返す必要がある。
        // そのため、未返却の貸出情報と返却済みの貸出情報をそれぞれ取得し、
        // 未返却の貸出情報があれば Vec に挿入して返す、という実装とする
        // 未返却の貸出情報を取得
        let checkout: Option<Checkout> = self.find_unreturned_by_book_id(book_id).await?;
        // 返却済みの貸出情報を取得
        let mut checkout_histories: Vec<Checkout> = sqlx::query_as!(
            ReturnedCheckoutRow,
            r#"
                SELECT
                rc.checkout_id,
                rc.book_id,
                rc.user_id,
                rc.checked_out_at,
                rc.returned_at,
                b.title,
                b.author,
                b.isbn
                FROM returned_checkouts AS rc
                INNER JOIN books AS b USING(book_id)
                WHERE rc.book_id = $1
                ORDER BY rc.checked_out_at DESC
            "#,
            book_id as _
        )
        .fetch_all(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?
        .into_iter()
        .map(Checkout::from)
        .collect();

        // 貸出中である場合は返却済みの履歴の先頭に追加する
        if let Some(co) = checkout {
            checkout_histories.insert(0, co);
        }

        Ok(checkout_histories)
    }
}

impl CheckoutRepositoryImpl {
    // create, update_returned メソッドでのトランザクションを利用するにあたり
    // トランザクション分離レベルを SERIALIZABLE にするために
    // 内部的に使うメソッド
    async fn set_transaction_serializable(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> AppResult<()> {
        sqlx::query!("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut **tx)
            .await
            .map_err(AppError::SpecificOperationError)?;

        Ok(())
    }

    // find_history_by_book_id で未返却の貸し出し情報を取得するために
    // 内部的に使うメソッド
    async fn find_unreturned_by_book_id(&self, book_id: BookId) -> AppResult<Option<Checkout>> {
        let res = sqlx::query_as!(
            CheckoutRow,
            r#"
                SELECT
                c.checkout_id,
                c.book_id,
                c.user_id,
                c.checked_out_at,
                b.title,
                b.author,
                b.isbn
                FROM checkouts AS c
                INNER JOIN books AS b USING(book_id)
                WHERE c.book_id = $1
            "#,
            book_id as _,
        )
        .fetch_optional(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?
        .map(Checkout::from);

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::Utc;
    use kernel::model::{checkout::CheckoutBook, id::BookId};

    use super::*;

    fn init_repo(pool: sqlx::PgPool) -> (CheckoutRepositoryImpl, UserId, UserId, BookId) {
        let repo = CheckoutRepositoryImpl::new(ConnectionPool::new(pool));

        // 事前登録したユーザー & 蔵書の ID (fixtures/checkout.sql参照)
        let user_id1 = UserId::from_str("9582f9de-0fd1-4892-b20c-70139a7eb95b").unwrap();
        let user_id2 = UserId::from_str("050afe56-c3da-4448-8e4d-6f44007d2ca5").unwrap();
        let book_id1 = BookId::from_str("9890736e-a4e4-461a-a77d-eac3517ef11b").unwrap();

        (repo, user_id1, user_id2, book_id1)
    }

    #[sqlx::test(fixtures("common", "checkout"))]
    async fn test_checkout_and_return(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let (repo, user_id1, user_id2, book_id1) = init_repo(pool);

        // 初期状態は貸出書籍は0
        let res = repo.find_unreturned_by_user_id(user_id1).await?;
        assert!(res.is_empty());

        let res = repo.find_unreturned_by_user_id(user_id2).await?;
        assert!(res.is_empty());

        let co = repo.find_unreturned_by_book_id(book_id1).await?;
        assert!(co.is_none());

        // 存在しない書籍への貸出は失敗する
        {
            let res = repo
                .create(CreateCheckout {
                    book_id: BookId::new(),
                    checked_out_by: user_id1,
                    checked_out_at: Utc::now(),
                })
                .await;
            assert!(matches!(res, Err(AppError::EntityNotFound(_))));
        }

        // book_id の蔵書を user_id1 に貸出
        {
            repo.create(CreateCheckout {
                book_id: book_id1,
                checked_out_by: user_id1,
                checked_out_at: Utc::now(),
            })
            .await?;

            let co = repo.find_unreturned_by_book_id(book_id1).await?;
            assert!(matches!(co, Some(
                    Checkout{
                        checked_out_by,
                        book:CheckoutBook { book_id, .. }, 
                        ..
                    }
                ) if book_id == book_id1 && checked_out_by == user_id1));

            // user_id1 の貸出中の状態で user_id2 への貸出が失敗する
            let res = repo
                .create(CreateCheckout {
                    book_id: book_id1,
                    checked_out_by: user_id2,
                    checked_out_at: Utc::now(),
                })
                .await;
            assert!(res.is_err());

            let co = co.unwrap();

            // 存在しない書籍への返却は失敗する
            let res = repo
                .update_returned(UpdateReturned {
                    checkout_id: co.id,
                    book_id: BookId::new(),
                    returned_by: user_id1,
                    returned_at: Utc::now(),
                })
                .await;
            assert!(matches!(res, Err(AppError::EntityNotFound(_))));

            // 存在しない checkoutId の貸出に対する返却は失敗する
            let res = repo
                .update_returned(UpdateReturned {
                    checkout_id: CheckoutId::new(),
                    book_id: book_id1,
                    returned_by: user_id1,
                    returned_at: Utc::now(),
                })
                .await;
            assert!(matches!(res, Err(AppError::UnprocessableEntiry(_))));

            // user_id1 への貸出で、ほかのユーザーからの返却は失敗する
            let res = repo
                .update_returned(UpdateReturned {
                    checkout_id: co.id,
                    book_id: book_id1,
                    returned_by: user_id2,
                    returned_at: Utc::now(),
                })
                .await;
            assert!(matches!(res, Err(AppError::UnprocessableEntiry(_))));

            // 成功する返却
            repo.update_returned(UpdateReturned {
                checkout_id: co.id,
                book_id: book_id1,
                returned_by: user_id1,
                returned_at: Utc::now(),
            })
            .await?;
        }
        Ok(())
    }

    #[sqlx::test(fixtures("common", "checkout"))]
    async fn test_checkout_list(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let (repo, user_id1, user_id2, book_id1) = init_repo(pool);

        // 初回の呼び出し
        {
            repo.create(CreateCheckout {
                book_id: book_id1,
                checked_out_by: user_id1,
                checked_out_at: Utc::now(),
            })
            .await?;

            let co = repo.find_unreturned_by_book_id(book_id1).await?.unwrap();

            // リスト出力
            {
                let res = repo.find_unreturned_all().await?;
                assert_eq!(res.len(), 1);

                let res = repo.find_unreturned_by_user_id(user_id1).await?;
                assert_eq!(res.len(), 1);

                let res = repo.find_unreturned_by_user_id(user_id2).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_history_by_book_id(book_id1).await?;
                assert_eq!(res.len(), 1);
            }

            repo.update_returned(UpdateReturned {
                checkout_id: co.id,
                book_id: book_id1,
                returned_by: user_id1,
                returned_at: Utc::now(),
            })
            .await?;

            // リストの出力
            {
                let res = repo.find_unreturned_all().await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_unreturned_by_user_id(user_id1).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_unreturned_by_user_id(user_id2).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_history_by_book_id(book_id1).await?;
                assert_eq!(res.len(), 1);
            }
        }

        // 2回目の貸出
        {
            repo.create(CreateCheckout {
                book_id: book_id1,
                checked_out_by: user_id2,
                checked_out_at: Utc::now(),
            })
            .await?;

            let co = repo.find_unreturned_by_book_id(book_id1).await?.unwrap();

            // リストの出力
            {
                let res = repo.find_unreturned_all().await?;
                assert_eq!(res.len(), 1);

                let res = repo.find_unreturned_by_user_id(user_id1).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_unreturned_by_user_id(user_id2).await?;
                assert_eq!(res.len(), 1);

                let res = repo.find_history_by_book_id(book_id1).await?;
                assert_eq!(res.len(), 2);
            }

            repo.update_returned(UpdateReturned {
                checkout_id: co.id,
                book_id: book_id1,
                returned_by: user_id2,
                returned_at: Utc::now(),
            })
            .await?;

            // リスト出力
            {
                let res = repo.find_unreturned_all().await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_unreturned_by_user_id(user_id1).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_unreturned_by_user_id(user_id2).await?;
                assert_eq!(res.len(), 0);

                let res = repo.find_history_by_book_id(book_id1).await?;
                assert_eq!(res.len(), 2);
            }
        }

        Ok(())
    }
}

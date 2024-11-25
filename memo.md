
```sh
curl -v "http://localhost:8080/auth/login" \
    -H 'content-type: application/json' \
    -d '{"email": "eleazar.fig@example.com", "password": "Pa55w0rd"}'
```

```sh
curl -v "http://localhost:8080/api/v1/users" \
    -H 'Authorization: Bearer ce491667798f4eef921753cecc5a1436' 
```

```sh
curl -v -X POST "http://localhost:8080/api/v1/users" \
    -H 'Authorization: Bearer ce491667798f4eef921753cecc5a1436' \
    -H 'Content-Type: application/json' \
    -d '{"name": "yamada", "email": "yamada@example.com", "password": "hogehoge"}'
```

本一覧取得
```sh
curl -v "http://localhost:8080/api/v1/books" \
    -H 'Authorization: Bearer e19e9f160ea8457cbaa29148d3863fbd'
```

本の登録
```sh
curl -v -X POST http://localhost:8080/api/v1/books \
    -H 'Authorization: Bearer 413e7a277f944f64a695cd4fc1efbd89' \
    -H 'Content-Type: application/json' \
    -d '{"title": "Rust book", "author": "me", "isbn": "1234567890", "description": ""}'

```

```sh
curl -v "http://localhost:8080/api/v1/books" \
    -H 'Authorization: Bearer 413e7a277f944f64a695cd4fc1efbd89' \
    -H 'Content-Type: application/json' | jq .
```

```sh
curl -v -X POST "http://localhost:8080/api/v1/books/a1a61547a9394907b245d52690f27846/checkouts" \
    -H 'authorization: Bearer a912833f0f3b47509e48b0517b7366a7'
```

```sh
curl -v "http://localhost:8080/api/v1/books/checkouts" \
    -H 'authorization: Bearer a912833f0f3b47509e48b0517b7366a7'
```

```sh
curl -v -X PUT "http://localhost:8080/api/v1/books/a1a61547a9394907b245d52690f27846/checkouts/b1b22539c5e147979af7fdd33732c201/returned" \
    -H 'authorization: Bearer a912833f0f3b47509e48b0517b7366a7' | jq
```

```sh
curl -v "http://localhost:8080/api/v1/books?offset=aaa" \
    -H 'authorization: Bearer b6a62a9f7b8d41068c05082580de9b20'
```


UIで借りる -> 返す -> 借りるとすると、次の返すでエラーになる
APIでも起きるかな？
returned_checkoutテーブルのbook_idにUNIQUE制約を間違って入れてた。
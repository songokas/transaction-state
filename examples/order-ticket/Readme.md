# Howto

run docker

```bash
docker-compose up -d
```

run migrations

```bash
# cargo install sqlx-cli
sqlx database create --database-url "postgres://postgres:123456@localhost/order_ticket"
sqlx database create --database-url "postgres://postgres:123456@localhost/order_ticket_test"
sqlx migrate run --database-url "postgres://postgres:123456@localhost/order_ticket"
sqlx migrate run --database-url "postgres://postgres:123456@localhost/order_ticket_test"
```

run example

```bash
cargo run --example order-ticket
```

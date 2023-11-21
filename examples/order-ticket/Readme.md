# Howto

run docker

```bash
docker run --name ticket-order --publish 5432:5432 -e POSTGRES_PASSWORD=123456 --rm -d postgres
docker run --name adminer --network host --rm -d adminer
```

run migrations

```bash
sqlx migrate run --database-url "postgres://postgres:123456@localhost/order_ticket"
```

run example

```bash
cargo run --example order-ticket
```


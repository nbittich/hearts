cargo install sqlx-cli
cargo sqlx prepare --database-url sqlite:/tmp/data.db
sqlx migrate run --database-url sqlite:/tmp/data.db
export DATABASE_URL="sqlite:/tmp/data.db"
cargo sqlx prepare --check --database-url sqlite:/tmp/data.db
sqlx database create --database-url sqlite:/tmp/data.db

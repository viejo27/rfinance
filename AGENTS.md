# AGENTS.md - Development Guide for rfinance_app

Guidelines for AI agents working on this codebase.

## Project Overview

- **Type**: Rust web application (actix-web 4.12.1)
- **Database**: PostgreSQL with sqlx 0.8.6
- **Templating**: askama 0.15.4
- **Edition**: Rust 2024

## Build, Lint, and Test Commands

```bash
# Build
cargo build              # debug build
cargo build --release    # release build

# Run (listens on 127.0.0.1:8080)
cargo run

# Tests
cargo test                              # run all tests
cargo test test_name                    # run single test by name
cargo test -- --nocapture               # show output
cargo test --test integration_tests     # run specific test file

# Linting
cargo clippy                            # lint
cargo clippy -- -D warnings             # warnings as errors
cargo fmt --check                       # check formatting
cargo fmt                               # format code
```

## Code Style Guidelines

### Naming Conventions
- Functions/variables: snake_case (`get_user`, `db_pool`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_CONNECTIONS`)
- Types/Enums: PascalCase (`User`, `TransactionType`)
- Files: snake_case (`home.rs`)

### Import Order
1. Standard library (`std`, `core`)
2. External crates (alphabetically)
3. Local modules

```rust
use std::env;
use actix_web::{App, HttpServer, web};
use sqlx::PgPool;
use crate::models::User;
```

### Error Handling
- Configuration/env errors: `expect()` with clear message
- Recoverable errors: `?` operator
- Expected errors: `match` or `if let`

```rust
let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env");
let result = query.fetch_one(&pool).await?;
```

### Async/Await
- Use `async` for route handlers
- Pass pools via `web::Data<T>`
- Clone pools in `move` closures

```rust
#[get("/")]
async fn index(db_pool: web::Data<PgPool>) -> impl Responder {
    let result = sqlx::query!("SELECT 1 as value")
        .fetch_one(db_pool.get_ref())
        .await
        .expect("Error executing query");
    HttpResponse::Ok().body(result.value.to_string())
}
```

### Templates (askama)
- Templates in `templates/` directory
- Use `#[derive(Template)]` and `#[template(path = "...")]`
- Pass references (`&str`, `&i32`)

```rust
#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> { name: &'a str, }
```

### Database (sqlx)
- Use `sqlx::query!` for compile-time checked queries
- Use `PgPool` for connection pooling
- Pass pool via `web::Data<PgPool>`, get reference with `db_pool.get_ref()`

### Database Migrations
The project uses sqlx which supports migrations. Run migrations before starting:
```bash
# Create migration (if needed)
sqlx migrate add migration_name
```

## Project Structure

```
rfinance_app/
├── Cargo.toml          # Dependencies
├── Cargo.lock
├── .env                # Environment (not committed)
├── .gitignore
├── src/
│   ├── main.rs         # Entry point
│   └── routes/web/
│       └── home.rs     # Route handlers
└── templates/
    └── index.html      # HTML templates
```

## Common Tasks

### Add a New Route
1. Create module in `src/routes/web/`
2. Declare in `src/main.rs`
3. Define handler with HTTP method attribute
4. Register route in `main.rs`

### Add a Dependency
1. Add to `[dependencies]` in `Cargo.toml`
2. Run `cargo fetch`
3. Import and use

## Configuration

`.env` file requires:
- `DATABASE_URL`: PostgreSQL connection string

## Notes

- All routes currently in `src/routes/web/home.rs`
- Requires PostgreSQL database
- Template errors fail at compile time
- Use `cargo clippy` before commits
- Never commit `.env` files

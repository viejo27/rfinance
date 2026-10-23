# AGENTS.md - Development Guide for rfinance_app

Guidelines for AI agents working on this codebase.

## Project Overview

- **Type**: Rust web application (actix-web 4.12.1)
- **Database**: PostgreSQL with sqlx 0.9.0
- **Templating**: askama 0.15.4
- **Edition**: Rust 2024

## Git Workflow

- **Never stage or commit without explicit approval.** Before any `git add`,
  `git commit`, `git push`, `git merge`, `git rebase` or `git reset`, list the
  files and the proposed commit message and wait for the user's confirmation.
- **Only commit on a branch you created for the current task.** If the working
  branch already existed (created or renamed by the user, or by a previous
  task), leave the changes uncommitted and ask first. Never add a commit to a
  branch you did not create.
- Never amend, force-push or rewrite history without explicit approval.
- Finished work stays in the working tree unless the user asks for a commit.

## Prerequisites

- Rust 1.94 or newer (MSRV of sqlx 0.9)
- A running PostgreSQL server with the target database created
- `sqlx-cli` only for authoring migrations (`sqlx migrate add`); the app applies
  migrations by itself at startup, see *Database Migrations*

A running database is needed **at build time** only when the `.sqlx/` cache
is missing or stale: `src/routes/web/admin.rs` uses the compile-time checked
`sqlx::query!` macro. The `.sqlx/` offline cache is committed, so regular
builds run with `SQLX_OFFLINE=true` and no database. Regenerate it with
`cargo sqlx prepare` (needs a live DB once) whenever that macro's query or
its schema change.

## Build, Lint, and Test Commands

```bash
# Build
cargo build              # debug build (requires DATABASE_URL reachable)
cargo build --release    # release build

# Run (listens on BIND_ADDR, default 127.0.0.1:8080)
cargo run

# Tests
cargo test                              # run all tests
cargo test test_name                    # run single test by name
cargo test -- --nocapture               # show output

# Linting
cargo clippy                            # lint
cargo clippy -- -D warnings             # warnings as errors
cargo fmt --check                       # check formatting
cargo fmt                               # format code
```

There are no automated tests in the repository yet. When adding some, prefer
`#[sqlx::test]` so each test gets an isolated, migrated database.

## Code Style Guidelines

### Naming: identifiers in English
- Every identifier is written in English: Rust and JS functions and
  variables, types, struct fields, module and file names, database tables,
  columns and indexes, CSS classes, DOM ids, API routes and JSON keys.
- User-facing copy (messages, labels, template text) and code comments
  stay in Spanish.
- Domain vocabulary (canonical translations): asiento/movimiento=transaction,
  cuenta=account, categoria=category, recurrencia=recurrence, cuota=installment,
  concepto=description, monto=amount, moneda=currency, fecha=entry_date,
  ingreso/egreso=income/expense, libro=ledger, presupuesto=budget, cartel=card,
  mando=overview, captura=entry, aviso=notice, rótulo=mini-label, celda=cell,
  fila=row, anulado=void, programado=scheduled, alcance=scope,
  saldo_inicial=opening_balance, limite=budget_limit, categoria_txt=category_label,
  es_ingreso=is_income, creada_en/creado_en=created_at, transferencia=transfer,
  pata=leg, tasa=rate, cuenta_origen=from_account, cuenta_destino=to_account.
- Never introduce a migration, column, class, id or identifier with a
  Spanish name; if you find a legacy one, rename it in a dedicated change.

### Public identifiers (API)
- Every resource exposed by the API is identified by `public_id` (UUID4),
  never by the internal serial id. Internal PKs/FKs stay `SERIAL` and are
  only used inside SQL joins and handlers.
- Every table exposed by the API carries
  `public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE`; the UNIQUE
  constraint creates the btree index used by `WHERE public_id = $1`.
- Routes take `web::Path<Uuid>`; handlers resolve the internal id
  (`SELECT id FROM <table> WHERE public_id = $1 AND user_id = $2`) before
  calling the pure logic, which keeps working with ints.
- Never expose serial ids in routes or JSON; JSON keys keep their names
  (`id`, `account_id`, `category_id`, `recurrence_id`) but carry UUIDs.
- `user_id` never travels to the frontend: it lives in the signed session
  and every query filters by `session_user_id(&session)`.

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
use crate::role::Role;
```

### Error Handling
- Configuration/env errors: `expect()` with clear message
- Recoverable errors: `?` operator
- Expected errors: `match` or `if let`

```rust
let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env");
let result = query.fetch_one(&pool).await?;
```

Startup-only code (`main.rs`, `ensure_admin_user`) may `expect()`, because a
failure there aborts the process anyway. Route handlers should not `unwrap()`
database errors: return a `500` instead.

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

The codebase mostly uses the **runtime** queries with a type annotation on the
result, which keeps the call site readable and avoids turbofish churn:

```rust
let existing: Option<(i32,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
    .bind(&email)
    .fetch_optional(pool)
    .await?;
```

- Use `sqlx::query!` / `sqlx::query_as!` (macros) when you want compile-time
  checked columns; they require a reachable database, or a generated `.sqlx/`
  offline cache
- Use `PgPool` for connection pooling
- Pass pool via `web::Data<PgPool>`, get reference with `db_pool.get_ref()`
- The `sqlx` dependency enables `runtime-async-std`, which sqlx 0.9 still
  supports. actix-web runs on tokio, so `runtime-tokio` is the more natural
  pairing; the async-std crate itself is deprecated upstream, and sqlx 0.9
  added `runtime-smol` and `runtime-async-global-executor` as alternatives

### Database Migrations

Migrations live in `migrations/` and are applied **automatically at startup** by
`sqlx::migrate!().run(&db_pool)` in `src/main.rs`, which runs before
`ensure_admin_user()`. A brand-new database needs no manual migration step.

Install the CLI to author or inspect migrations:

```bash
# Arch / CachyOS: prebuilt binary, same version as the library in Cargo.toml
paru -S sqlx-cli

# Building from source instead: pin the version to Cargo.toml, and skip the
# drivers this project does not use (default features add mysql, sqlite,
# native-tls and completions)
cargo install sqlx-cli --version 0.9.0 --no-default-features --features postgres,rustls
```

```bash
sqlx migrate info            # show applied and pending migrations
sqlx migrate run             # apply pending migrations without starting the app
sqlx migrate add <name> -r   # author a new migration as an up/down pair
sqlx migrate revert          # run the .down.sql of the most recent migration
```

- The CLI reads `DATABASE_URL` from `.env` automatically
- Keep `sqlx-cli` and the `sqlx` crate on the same version: the `.sqlx/` cache
  produced by `sqlx prepare` is not portable between versions
- Naming convention is `<TIMESTAMP>_<description>.up.sql` plus `.down.sql`;
  keep every migration reversible so `sqlx migrate revert` always works
- `sqlx::migrate!()` embeds the directory into the binary at compile time, so
  rebuild after adding or editing a migration
- Never edit a migration that is already applied: sqlx compares checksums and
  fails with `VersionMismatch`

## Project Structure

```
rfinance_app/
├── Cargo.toml              # Dependencies
├── Cargo.lock
├── .env                    # Local config (gitignored, never commit)
├── .env.example            # Template for .env
├── migrations/             # sqlx migrations (up/down pairs)
├── src/
│   ├── main.rs             # Pool, migrations, admin bootstrap, HTTP server
│   ├── role.rs             # `Role` enum (Admin/User) parsed from the session
│   ├── middleware/
│   │   ├── auth.rs         # `check_session`: redirects and public paths
│   │   └── grants.rs       # `extract`: role extractor for actix-web-grants
│   └── routes/
│       ├── mod.rs
│       ├── api/            # JSON endpoints: login, register, logout
│       └── web/            # HTML endpoints: home, login, register, admin
├── static/
│   ├── css/style.css
│   └── js/                 # home.js, login.js, register.js
└── templates/              # index.html, login.html, register.html, admin.html
```

## Common Tasks

### Add a New Route
1. Create the handler module in `src/routes/web/` (HTML) or `src/routes/api/` (JSON)
2. Declare it with `pub mod` in that directory's `mod.rs`
3. Re-export it with a `*_handler` / `api_*` alias in the same `mod.rs`
4. Register the service with `.service(...)` in `src/main.rs`
5. If the path must bypass the session redirect, add it to `middleware/auth.rs`

### Add a Dependency
1. Add to `[dependencies]` in `Cargo.toml`
2. Run `cargo fetch`
3. Import and use

## Configuration

`.env` file requires (copy from `.env.example`):
- `DATABASE_URL`: PostgreSQL connection string
- `BIND_ADDR`: listen address as `IP:PORT`, default `127.0.0.1:8080`.
  `0.0.0.0:8080` exposes the app to the LAN over plain HTTP (no TLS): trusted
  networks only, session cookies are replayable if sniffed (see `SECURITY.md`)
- `ADMIN_USER`: email of the account `ensure_admin_user` creates and promotes
- `ADMIN_PASSWORD`: password for that account, only used when creating it
- `SESSION_KEY`: at least 32 raw bytes, used to encrypt and sign the session
  cookie (`openssl rand -hex 32`). Rotating it invalidates every session
  (all users are logged out); see `SECURITY.md` for the rotation runbook.
- `APP_ENV`: `production` enables secure cookies, anything else is development

## Notes

- Web (HTML) routes live in `src/routes/web/`, JSON routes in `src/routes/api/`
- Requires a PostgreSQL database, running even for `cargo build`
- Template errors fail at compile time
- Use `cargo clippy` before commits
- Never commit `.env` files

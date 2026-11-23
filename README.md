# rfinance_app

Personal finance web app built with Rust, actix-web, PostgreSQL and sqlx.

## Requirements

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.94+ | MSRV of sqlx 0.9 |
| PostgreSQL | running, with a database created | also needed at **build** time |
| sqlx-cli | 0.9.x | only for authoring migrations |

The PostgreSQL server only has to be reachable, the database can be empty: the
app creates its own schema on boot.

## Setup

```bash
# 1. Configure the environment
cp .env.example .env

# 2. Create the database
createdb rfinance

# 3. Install the migration CLI (Arch / CachyOS)
paru -S sqlx-cli
```

Then edit `.env`:

| Variable | Description |
| --- | --- |
| `DATABASE_URL` | Connection string, e.g. `postgres://user:password@localhost/rfinance` |
| `ADMIN_USER` | Email of the admin account the app creates on first run |
| `ADMIN_PASSWORD` | Password for that account (see *Admin account* below) |
| `SESSION_KEY` | Cookie signing key, generate with `openssl rand -hex 64` |
| `APP_ENV` | `production` marks session cookies as secure; any other value is development |

`.env` is gitignored; never commit real credentials.

## Run

```bash
cargo run
```

Open <http://127.0.0.1:8080>.

On the very first run the server connects, applies every pending migration, and
makes sure the admin account exists before it starts listening.

## Database and migrations

Migrations are stored in `migrations/` as reversible `up`/`down` pairs and are
applied automatically at startup by `sqlx::migrate!()` in `src/main.rs`. For
day-to-day work you rarely need the CLI.

```bash
sqlx migrate info              # what is applied and what is pending
sqlx migrate run               # apply pending migrations without starting the app
sqlx migrate add <name> -r     # author a new migration (creates .up.sql and .down.sql)
sqlx migrate revert            # undo the most recent migration
```

Both the CLI and the app read `DATABASE_URL` from `.env`.

Two rules worth knowing:

- `sqlx::migrate!()` embeds `migrations/` into the binary at compile time, so
  rebuild after adding a migration.
- Never modify a migration that has already been applied: sqlx compares
  checksums and refuses to start. Create a new one instead.

### Admin account

`ensure_admin_user()` runs after the migrations. It looks for a user whose email
matches `ADMIN_USER`; if it finds none, it creates one (bcrypt hashed) and marks
it as `admin` in `user_data`. If the account already exists it only reasserts
the role, it does **not** reset the password. To change the admin password,
either set it on a fresh database or update the row directly.

## Correo transaccional

El alta envía un correo de bienvenida con el enlace de verificación; el login
permite entrar sin verificar (se muestra un aviso en el panel), y
`/forgot-password` manda un enlace de reset válido 60 minutos. Todo pasa por
una tabla outbox (`email_outbox`) y un worker en el mismo proceso: los
handlers nunca bloquean esperando al SMTP y los fallos se reintentan con
backoff.

Variables en `.env`:

| Variable | Descripción |
| --- | --- |
| `MAIL_TRANSPORT` | `smtp`, `log` (imprime en stdout, no envía) o `disabled` |
| `MAIL_FROM` | Remitente, por ejemplo `RFinance <no-reply@tudominio.com>` |
| `MAIL_REPLY_TO` | Reply-To; si falta usa `MAIL_FROM` |
| `SMTP_HOST` / `SMTP_PORT` | Relay SMTP |
| `SMTP_TLS` | `wrapper` (465), `starttls` (587) o `none` (solo dev) |
| `SMTP_USER` / `SMTP_PASS` | Credenciales del buzón; vacías solo para relays locales sin auth |
| `APP_URL` | URL pública que se usa en los enlaces de los correos |

En desarrollo, `MAIL_TRANSPORT=log` deja ver el flujo sin tocar la red (el
cuerpo completo sale en nivel `debug`). Para probar la entrega real contra
un SMTP local, Mailpit escucha en `127.0.0.1:1025` con
`SMTP_TLS=none` y credenciales vacías.

Con `MAIL_TRANSPORT=smtp`, `POST /admin/mail-test` (solo admin) encola un
correo a la casilla del propio administrador: es la prueba de humo de
credenciales, TLS y entregabilidad. El estado de cada envío queda en
`email_outbox.status` (`pending`/`sending`/`sent`/`failed`) con `last_error`.

El reset de contraseña sube `users.session_epoch`; el middleware compara ese
valor contra el epoch guardado en el cookie y cierra el resto de las sesiones
en el siguiente request. Los tokens de verificación y reset se guardan
hasheados (SHA-256) y son de un solo uso.

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/` | Home, requires a session |
| GET | `/login` | Login page |
| GET | `/register` | Registration page |
| GET | `/forgot-password` | Request a password reset link |
| GET | `/reset-password?token=…` | Set a new password |
| GET | `/verify-email?token=…` | Verify the account email |
| GET | `/admin` | Admin page, restricted to the `Admin` role |
| POST | `/admin/mail-test` | Enqueue a test email to the admin's own mailbox |
| POST | `/api/login` | Authenticate and open a session |
| POST | `/api/register` | Create a user and enqueue the welcome email |
| POST | `/api/logout` | Purge the session |
| POST | `/api/forgot-password` | Enqueue a reset link (always a generic 200) |
| POST | `/api/reset-password` | Consume a reset token and rotate the password |
| POST | `/api/resend-verification` | Re-send the verification link (session) |

Unauthenticated requests to protected paths are redirected to `/login` by
`src/middleware/auth.rs`; `/admin` access is enforced with `actix-web-grants`.

## Development

```bash
cargo build              # debug build
cargo clippy             # lints, project convention is to keep these clean
cargo fmt                # format
cargo test               # unit + DB integration tests (database must be up)
```

Template errors and `sqlx::query!` column mismatches fail at compile time, which
is why the build needs a reachable database. See `AGENTS.md` for the full
conventions and project layout.

## Project layout

```
src/
├── main.rs              # pool, migrations, admin bootstrap, HTTP server
├── role.rs              # Role enum used for authorization
├── mail/                # SMTP/log, outbox + worker, tokens y plantillas
├── middleware/          # auth.rs (session redirect), grants.rs (role extractor)
└── routes/
    ├── api/             # JSON endpoints: login, register, logout, reset
    └── web/             # HTML endpoints: home, login, register, admin
static/                  # css/style.css, js/*.js
templates/               # askama HTML templates (email/ para los correos)
migrations/              # sqlx up/down pairs
```

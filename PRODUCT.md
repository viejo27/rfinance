# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

The general public, with Latin America as the initial audience: people whose money
life spans multiple currencies every day (e.g. pesos for spending, USD for saving)
and whom mainstream finance apps serve poorly. The builder is the first user — the
app must prove itself as their daily finance driver before strangers rely on it.
Administrators (the builder and future operators) are a secondary audience working
in the back office.

## Product Purpose

RFinance is a web app where registered users record and understand their money:
income and expenses, accounts and wallets, budgets, recurring transactions, debts
and loans, savings goals, and investments, with reports and charts on top. It exists
to give LatAm users a finance tool built for their actual multi-currency,
inflation-shaped reality. Success means: a trusted daily driver first (the builder's
own finances run on it), public adoption after.

## Positioning

LatAm-first: bilingual ES/EN and multi-currency at the core, built for dollarized
and inflation-shaped economies. The reality mainstream finance apps treat as an edge
case is RFinance's default scenario.

## Operating Context

- Browser-based daily use: quick everyday transaction entry; periodic review of
  budgets, reports, and goals.
- Users hold cash, bank accounts, cards, and wallets in different currencies at the
  same time.
- Public product: strangers register with email + password; sessions last 27 days;
  an admin back office at `/admin` covers user management, platform metrics, content
  moderation, and support tooling as the product grows.
- Self-hosted single Rust binary plus PostgreSQL; migrations apply at startup;
  PostgreSQL must also be reachable at build time.

## Capabilities and Constraints

Existing (implemented):
- Authentication: register (email, password, name, last name), login, logout;
  bcrypt hashing; signed cookie sessions.
- Roles: Admin / User via actix-web-grants; the admin account is bootstrapped from
  `.env` (`ADMIN_USER` / `ADMIN_PASSWORD`).
- Income/expense ledger with categories, accounts & wallets with per-currency
  balances, budgets & limits, recurring transactions and transfers.
- Investments strategy surface at `/inversiones`: risk profiles, mixed
  fixed-income/equity assets, composition by class/economy/sector, 11 weighted
  checks, score, rebalancing orders, target repartition by RV/RF sleeve
  (percentages only, with preview), a liquidity asset as the counterpart for
  executing orders and sales, and written sell triggers. Values are entered manually in
  ARS or USD; MEP/CCL rates come from DolarAPI and the view can be ARS·MEP,
  ARS·CCL, USD·MEP or USD·CCL.
- Stack: Rust 2024 (MSRV 1.94), actix-web 4.12, sqlx 0.9 + PostgreSQL, askama
  templates, vanilla JS, no frontend framework.
- Schema: `users`, `user_data`, `accounts`, `categories`, `transactions`,
  `transfers`, `recurrences`, `portfolio_settings`, `assets`, `sell_triggers`,
  `email_outbox`.

Planned full suite (confirmed scope):
- Reports & charts; debts & loans; savings goals; live quotes for the
  investment portfolio (BYMA Open Data in a later phase, with the last reading
  cached because the free tier does not answer weekends or holidays).

Constraints and undecided facts:
- The UI must be bilingual ES/EN; no i18n mechanism exists yet (copy is currently
  Spanish in the shipped surfaces).
- Multi-currency: every amount carries its currency. The dashboard does not
  convert between ARS and USD; the investments surface converts with the MEP or
  CCL rate explicitly chosen by the user, and never invents a rate.
- Investment values are manual in phase 1; live quote providers are a later
  phase.
- Automated tests cover the new portfolio engine and the existing finance flows;
  conventions are clippy-clean, fmt'ed code.
- Whether the unused profile fields (nick_name, age, gender, ip, user_agent) become
  product features is undecided.
- Deployment target beyond local `127.0.0.1:8080` is undecided.

## Brand Commitments

- Name: "RFinance" (as used on the login page). The package name `rfinance_app`
  still leaks into some titles; final naming consistency is unconfirmed.
- No logo, visual identity, or voice guidelines exist yet. The current placeholder
  look (warm beige palette) is scaffolding, not a committed visual world.
- Binding visual constraint set by the owner: Soviet constructivism + utilitarian
  brutalism, sober (no parody or kitsch), for the first real surfaces. Full world
  detailing belongs to the surface brief and DESIGN.md, not to this record.

## Evidence on Hand

- None: no logo, marketing assets, testimonials, case studies, press, or seed data.
  Future work must not fabricate any.
- The first real content will be the builder's own financial data once the app
  becomes their daily driver.

## Product Principles

1. Earn daily-driver trust: correctness, data safety, and fast everyday entry beat
   feature breadth; the builder's own money is the acceptance test.
2. Multi-currency is first-class: every amount carries its currency; peso-and-dollar
   life is the default scenario, never an edge case.
3. Spanish is a first-class language: ES and EN ship together; neither is an
   afterthought translation.
4. One system, not isolated screens: accounts feed budgets, budgets feed reports;
   the suite must cohere.
5. Run it like a real product: the admin back office keeps user management, metrics,
   moderation, and support visible and actionable.

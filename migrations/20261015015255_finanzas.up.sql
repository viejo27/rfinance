-- Finanzas: cuentas, categorías, recurrencias y movimientos.
-- Todo cuelga del usuario de sesión (user_id). Montos en NUMERIC(14,2).
-- Orden: los movimientos referencian cuentas, categorías y recurrencias.

CREATE TABLE accounts (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	name VARCHAR(100) NOT NULL,
	currency VARCHAR(3) NOT NULL CHECK (currency IN ('ARS', 'USD')),
	opening_balance NUMERIC(14, 2) NOT NULL DEFAULT 0,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	UNIQUE (user_id, name)
);

CREATE INDEX idx_accounts_user ON accounts(user_id);

CREATE TABLE categories (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	name VARCHAR(60) NOT NULL,
	budget_limit NUMERIC(14, 2),
	currency VARCHAR(3) NOT NULL DEFAULT 'ARS',
	UNIQUE (user_id, name)
);

CREATE INDEX idx_categories_user ON categories(user_id);

CREATE TABLE recurrences (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
	category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
	category_label VARCHAR(60) NOT NULL,
	description VARCHAR(200) NOT NULL,
	amount NUMERIC(14, 2) NOT NULL CHECK (amount > 0),
	is_income BOOLEAN NOT NULL,
	currency VARCHAR(3) NOT NULL CHECK (currency IN ('ARS', 'USD')),
	day_of_month SMALLINT NOT NULL CHECK (day_of_month BETWEEN 1 AND 28),
	start_date DATE NOT NULL,
	installment_count SMALLINT,
	installments_generated SMALLINT NOT NULL DEFAULT 0,
	is_active BOOLEAN NOT NULL DEFAULT true,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_recurrences_user ON recurrences(user_id);

CREATE TABLE transactions (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
	category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
	category_label VARCHAR(60) NOT NULL,
	description VARCHAR(200) NOT NULL,
	amount NUMERIC(14, 2) NOT NULL CHECK (amount > 0),
	is_income BOOLEAN NOT NULL,
	currency VARCHAR(3) NOT NULL CHECK (currency IN ('ARS', 'USD')),
	entry_date DATE NOT NULL,
	recurrence_id INTEGER REFERENCES recurrences(id) ON DELETE SET NULL,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_transactions_user_date ON transactions(user_id, entry_date);
CREATE INDEX idx_transactions_recurrence ON transactions(recurrence_id);

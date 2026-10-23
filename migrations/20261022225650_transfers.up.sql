-- Transferencias entre cuentas: movimiento propio, separado del libro de
-- asientos. Cada monto vive en la moneda de su pata (la de su cuenta); la
-- tasa no se persiste, se deriva de to_amount / from_amount.
-- El ON DELETE CASCADE de las cuentas arrastra la transferencia completa.

CREATE TABLE transfers (
	id SERIAL PRIMARY KEY,
	public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	from_account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
	to_account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
	from_amount NUMERIC(14, 2) NOT NULL CHECK (from_amount > 0),
	from_currency VARCHAR(3) NOT NULL CHECK (from_currency IN ('ARS', 'USD')),
	to_amount NUMERIC(14, 2) NOT NULL CHECK (to_amount > 0),
	to_currency VARCHAR(3) NOT NULL CHECK (to_currency IN ('ARS', 'USD')),
	entry_date DATE NOT NULL,
	description VARCHAR(200) NOT NULL,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	CHECK (from_account_id <> to_account_id)
);

CREATE INDEX idx_transfers_user_date ON transfers(user_id, entry_date);

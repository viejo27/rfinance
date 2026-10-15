-- Finanzas: cuentas, categorías, recurrencias y movimientos.
-- Todo cuelga del usuario de sesión (user_id). Montos en NUMERIC(14,2).
-- Orden: movimientos referencia a cuentas, categorías y recurrencias.

CREATE TABLE cuentas (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	nombre VARCHAR(100) NOT NULL,
	moneda VARCHAR(3) NOT NULL CHECK (moneda IN ('ARS', 'USD')),
	saldo_inicial NUMERIC(14, 2) NOT NULL DEFAULT 0,
	creada_en TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	UNIQUE (user_id, nombre)
);

CREATE INDEX idx_cuentas_user ON cuentas(user_id);

CREATE TABLE categorias (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	nombre VARCHAR(60) NOT NULL,
	limite NUMERIC(14, 2),
	moneda VARCHAR(3) NOT NULL DEFAULT 'ARS',
	UNIQUE (user_id, nombre)
);

CREATE INDEX idx_categorias_user ON categorias(user_id);

CREATE TABLE recurrencias (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	cuenta_id INTEGER NOT NULL REFERENCES cuentas(id) ON DELETE CASCADE,
	categoria_id INTEGER REFERENCES categorias(id) ON DELETE SET NULL,
	categoria_txt VARCHAR(60) NOT NULL,
	concepto VARCHAR(200) NOT NULL,
	monto NUMERIC(14, 2) NOT NULL CHECK (monto > 0),
	es_ingreso BOOLEAN NOT NULL,
	moneda VARCHAR(3) NOT NULL CHECK (moneda IN ('ARS', 'USD')),
	dia SMALLINT NOT NULL CHECK (dia BETWEEN 1 AND 28),
	inicio DATE NOT NULL,
	cuotas SMALLINT,
	cuotas_generadas SMALLINT NOT NULL DEFAULT 0,
	activa BOOLEAN NOT NULL DEFAULT true,
	creada_en TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_rec_user ON recurrencias(user_id);

CREATE TABLE movimientos (
	id SERIAL PRIMARY KEY,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	cuenta_id INTEGER NOT NULL REFERENCES cuentas(id) ON DELETE CASCADE,
	categoria_id INTEGER REFERENCES categorias(id) ON DELETE SET NULL,
	categoria_txt VARCHAR(60) NOT NULL,
	concepto VARCHAR(200) NOT NULL,
	monto NUMERIC(14, 2) NOT NULL CHECK (monto > 0),
	es_ingreso BOOLEAN NOT NULL,
	moneda VARCHAR(3) NOT NULL CHECK (moneda IN ('ARS', 'USD')),
	fecha DATE NOT NULL,
	rec_id INTEGER REFERENCES recurrencias(id) ON DELETE SET NULL,
	creado_en TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_mov_user_fecha ON movimientos(user_id, fecha);
CREATE INDEX idx_mov_rec ON movimientos(rec_id);

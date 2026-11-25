-- Cartera de inversiones: criterios de la estrategia, activos y
-- disparadores de venta. Los valores se cargan a mano en la fase 1;
-- las columnas de mercado (source, symbol, quantity, last_price) quedan
-- listas para la fase de cotizaciones sin rehacer las tablas.

CREATE TABLE portfolio_settings (
	id SERIAL PRIMARY KEY,
	public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
	user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
	risk_profile VARCHAR(20) NOT NULL DEFAULT 'aggressive'
		CHECK (risk_profile IN ('conservative', 'moderate', 'aggressive')),
	valuation VARCHAR(10) NOT NULL DEFAULT 'usd_ccl'
		CHECK (valuation IN ('ars_mep', 'ars_ccl', 'usd_mep', 'usd_ccl')),
	target_fixed_income_pct NUMERIC(5, 2) NOT NULL DEFAULT 40
		CHECK (target_fixed_income_pct BETWEEN 0 AND 100),
	target_equity_pct NUMERIC(5, 2) NOT NULL DEFAULT 60
		CHECK (target_equity_pct BETWEEN 0 AND 100),
	rebalance_band_pp NUMERIC(5, 2) NOT NULL DEFAULT 5
		CHECK (rebalance_band_pp BETWEEN 0 AND 100),
	min_assets SMALLINT NOT NULL DEFAULT 8 CHECK (min_assets >= 0),
	max_assets SMALLINT NOT NULL DEFAULT 13 CHECK (max_assets >= min_assets),
	max_asset_pct NUMERIC(5, 2) NOT NULL DEFAULT 15
		CHECK (max_asset_pct BETWEEN 0 AND 100),
	max_economy_pct NUMERIC(5, 2) NOT NULL DEFAULT 70
		CHECK (max_economy_pct BETWEEN 0 AND 100),
	max_cluster_pct NUMERIC(5, 2) NOT NULL DEFAULT 35
		CHECK (max_cluster_pct BETWEEN 0 AND 100),
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	CHECK (target_fixed_income_pct + target_equity_pct = 100)
);

CREATE TABLE assets (
	id SERIAL PRIMARY KEY,
	public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	ticker VARCHAR(20),
	name VARCHAR(120) NOT NULL,
	asset_class VARCHAR(20) NOT NULL
		CHECK (asset_class IN ('fixed_income', 'equity', 'mixed')),
	fixed_income_pct NUMERIC(5, 2) NOT NULL DEFAULT 0
		CHECK (fixed_income_pct BETWEEN 0 AND 100),
	economy VARCHAR(20) NOT NULL
		CHECK (economy IN ('us', 'europe', 'emerging', 'argentina', 'global')),
	sector VARCHAR(30) NOT NULL
		CHECK (sector IN (
			'technology', 'communication_services', 'consumer_discretionary',
			'consumer_staples', 'financials', 'industrials', 'materials',
			'energy', 'health_care', 'utilities', 'real_estate', 'other'
		)),
	market_value NUMERIC(14, 2) NOT NULL DEFAULT 0 CHECK (market_value >= 0),
	currency VARCHAR(3) NOT NULL CHECK (currency IN ('ARS', 'USD')),
	target_weight_pct NUMERIC(5, 2) NOT NULL DEFAULT 0
		CHECK (target_weight_pct BETWEEN 0 AND 100),
	thesis TEXT NOT NULL DEFAULT '',
	objective TEXT NOT NULL DEFAULT '',
	source VARCHAR(10) NOT NULL DEFAULT 'manual'
		CHECK (source IN ('manual', 'market')),
	market_symbol VARCHAR(30),
	quantity NUMERIC(20, 6) CHECK (quantity IS NULL OR quantity >= 0),
	quote_unit SMALLINT NOT NULL DEFAULT 1 CHECK (quote_unit IN (1, 100)),
	last_price NUMERIC(18, 6) CHECK (last_price IS NULL OR last_price >= 0),
	price_currency VARCHAR(3)
		CHECK (price_currency IS NULL OR price_currency IN ('ARS', 'USD')),
	priced_at TIMESTAMP,
	is_active BOOLEAN NOT NULL DEFAULT true,
	sort_order SMALLINT NOT NULL DEFAULT 0,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	CHECK (
		(asset_class = 'fixed_income' AND fixed_income_pct = 100)
		OR (asset_class = 'equity' AND fixed_income_pct = 0)
		OR (asset_class = 'mixed' AND fixed_income_pct > 0 AND fixed_income_pct < 100)
	)
);

CREATE INDEX idx_assets_user ON assets(user_id, is_active);

CREATE TABLE sell_triggers (
	id SERIAL PRIMARY KEY,
	public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
	user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
	kind VARCHAR(20) NOT NULL
		CHECK (kind IN ('thesis_broken', 'fundamentals', 'target_met', 'rebalance', 'other')),
	description VARCHAR(300) NOT NULL,
	is_met BOOLEAN NOT NULL DEFAULT false,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_sell_triggers_user ON sell_triggers(user_id);
CREATE INDEX idx_sell_triggers_asset ON sell_triggers(asset_id);

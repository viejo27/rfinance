-- La liquidez es la contraparte del rebalanceo: recibe las ventas y
-- paga las compras. Un usuario puede marcar como máximo un activo.
ALTER TABLE assets
    ADD COLUMN is_cash BOOLEAN NOT NULL DEFAULT false;

CREATE UNIQUE INDEX idx_assets_one_cash
    ON assets(user_id)
    WHERE is_cash;

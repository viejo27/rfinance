-- Identificador público UUID4 por recurso de dominio.
-- Los ids seriales quedan como claves internas (FKs y joins); la API
-- expone únicamente public_id. El UNIQUE crea el índice btree único
-- que usan las búsquedas WHERE public_id = $1.

ALTER TABLE accounts
    ADD COLUMN public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE;

ALTER TABLE categories
    ADD COLUMN public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE;

ALTER TABLE recurrences
    ADD COLUMN public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE;

ALTER TABLE transactions
    ADD COLUMN public_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE;

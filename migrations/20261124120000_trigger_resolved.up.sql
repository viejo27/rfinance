-- Un disparador cumplido puede ejecutarse (vender el activo y cerrar el
-- aviso) sin borrar el registro: `is_met` marca la condición y
-- `is_resolved` marca que la acción ya se tomó.
ALTER TABLE sell_triggers
    ADD COLUMN is_resolved BOOLEAN NOT NULL DEFAULT false;

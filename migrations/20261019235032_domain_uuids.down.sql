ALTER TABLE transactions DROP COLUMN IF EXISTS public_id;
ALTER TABLE recurrences DROP COLUMN IF EXISTS public_id;
ALTER TABLE categories DROP COLUMN IF EXISTS public_id;
ALTER TABLE accounts DROP COLUMN IF EXISTS public_id;

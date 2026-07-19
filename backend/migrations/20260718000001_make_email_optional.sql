-- Email is optional until #83 (email verification) lands, so a user can
-- register without one. The UNIQUE constraint is kept: Postgres treats
-- NULLs as distinct under UNIQUE, so multiple emailless rows coexist and
-- collisions between real addresses are still caught.
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;

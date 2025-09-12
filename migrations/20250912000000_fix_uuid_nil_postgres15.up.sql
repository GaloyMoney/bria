-- Fix for PostgreSQL 15 compatibility: Define uuid_nil() function if it doesn't exist
-- In PostgreSQL 15, uuid_nil() may not be available even with uuid-ossp extension

-- Patch: Ensure uuid_nil() exists for Postgres 15+ without uuid-ossp

-- Patch: Always (re)define uuid_nil() for Postgres 14/15 compatibility
CREATE OR REPLACE FUNCTION uuid_nil()
RETURNS uuid AS $$
    SELECT '00000000-0000-0000-0000-000000000000'::uuid;
$$ LANGUAGE SQL IMMUTABLE;

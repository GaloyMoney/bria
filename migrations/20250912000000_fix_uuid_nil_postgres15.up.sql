-- Fix for PostgreSQL 15 compatibility: Define uuid_nil() function if it doesn't exist
-- In PostgreSQL 15, uuid_nil() may not be available even with uuid-ossp extension

-- Create the function only if it doesn't already exist
DO $$
BEGIN
    -- Check if uuid_nil function exists
    IF NOT EXISTS (
        SELECT 1 
        FROM pg_proc 
        WHERE proname = 'uuid_nil'
    ) THEN
        -- Create uuid_nil function that returns the nil UUID
        CREATE FUNCTION uuid_nil() RETURNS uuid
        AS 'SELECT ''00000000-0000-0000-0000-000000000000''::uuid'
        LANGUAGE SQL IMMUTABLE;
        
        COMMENT ON FUNCTION uuid_nil() IS 'Returns the nil UUID (all zeros)';
    END IF;
END
$$;
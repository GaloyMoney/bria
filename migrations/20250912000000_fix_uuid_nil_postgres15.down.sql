-- Rollback: Remove the custom uuid_nil function if we added it
-- Only drop if it's our custom function (not from an extension)
DO $$
BEGIN
    -- Check if uuid_nil function exists and is not from an extension
    IF EXISTS (
        SELECT 1 
        FROM pg_proc p
        LEFT JOIN pg_depend d ON d.objid = p.oid
        LEFT JOIN pg_extension e ON e.oid = d.refobjid
        WHERE p.proname = 'uuid_nil'
        AND e.extname IS NULL  -- Not part of an extension
    ) THEN
        DROP FUNCTION IF EXISTS uuid_nil();
    END IF;
END
$$;
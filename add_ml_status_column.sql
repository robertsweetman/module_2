-- Migration script to add ml_status column to tender_records table
-- Run this via bastion host or psql connection to RDS

-- Add ml_status column if it doesn't exist
DO $$
BEGIN
    -- Check if ml_status column exists
    IF NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_name = 'tender_records'
        AND column_name = 'ml_status'
    ) THEN
        -- Add the column
        ALTER TABLE tender_records
        ADD COLUMN ml_status VARCHAR(20) DEFAULT 'pending';

        RAISE NOTICE 'Successfully added ml_status column';
    ELSE
        RAISE NOTICE 'ml_status column already exists';
    END IF;
END $$;

-- Verify the column was added
SELECT column_name, data_type, character_maximum_length, column_default
FROM information_schema.columns
WHERE table_name = 'tender_records'
AND column_name = 'ml_status';

-- Check current state of ML-related columns
SELECT column_name, data_type, is_nullable, column_default
FROM information_schema.columns
WHERE table_name = 'tender_records'
AND column_name LIKE 'ml_%'
ORDER BY column_name;

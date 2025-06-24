-- ROLLBACK SCRIPT: Use this if you need to revert the date columns back to TEXT
-- WARNING: This will convert all dates back to text format

-- Step 1: Convert date columns back to TEXT
ALTER TABLE tender_records ALTER COLUMN published TYPE TEXT;
ALTER TABLE tender_records ALTER COLUMN deadline TYPE TEXT;
ALTER TABLE tender_records ALTER COLUMN awarddate TYPE TEXT;

-- Step 2: If you have backup data, you can restore it like this:
-- UPDATE tender_records SET 
--     published = backup.published,
--     deadline = backup.deadline,
--     awarddate = backup.awarddate
-- FROM tender_records_backup backup
-- WHERE tender_records.resource_id = backup.resource_id;

-- Step 3: Re-add NOT NULL constraints if they were there originally
-- (Only uncomment if your original schema had NOT NULL on these columns)
-- ALTER TABLE tender_records ALTER COLUMN published SET NOT NULL;
-- ALTER TABLE tender_records ALTER COLUMN deadline SET NOT NULL;
-- ALTER TABLE tender_records ALTER COLUMN awarddate SET NOT NULL; 
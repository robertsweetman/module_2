-- Database Migration Script for Tender Records (In-Place Updates)
-- Run this script in pgAdmin4 to update the table schema safely

-- Step 1: Add the BID column for ML labelling (if it doesn't exist)
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'bid') THEN
        ALTER TABLE tender_records ADD COLUMN bid INTEGER DEFAULT NULL;
        COMMENT ON COLUMN tender_records.bid IS 'ML label: 1 if we should bid, 0 if not, NULL if not yet labelled';
    END IF;
END $$;

-- Step 1b: Add ML prediction and processing columns
DO $$ 
BEGIN
    -- Add ml_processed column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'ml_processed') THEN
        ALTER TABLE tender_records ADD COLUMN ml_processed BOOLEAN DEFAULT FALSE;
        COMMENT ON COLUMN tender_records.ml_processed IS 'Whether this tender has been processed by ML predictor';
    END IF;
    
    -- Add ml_bid column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'ml_bid') THEN
        ALTER TABLE tender_records ADD COLUMN ml_bid BOOLEAN DEFAULT NULL;
        COMMENT ON COLUMN tender_records.ml_bid IS 'ML prediction: TRUE if we should bid, FALSE if not, NULL if not predicted';
    END IF;
    
    -- Add ml_confidence column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'ml_confidence') THEN
        ALTER TABLE tender_records ADD COLUMN ml_confidence DECIMAL(5,4) DEFAULT NULL;
        COMMENT ON COLUMN tender_records.ml_confidence IS 'ML prediction confidence score (0.0000 to 1.0000)';
    END IF;
    
    -- Add ml_reasoning column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'ml_reasoning') THEN
        ALTER TABLE tender_records ADD COLUMN ml_reasoning TEXT DEFAULT NULL;
        COMMENT ON COLUMN tender_records.ml_reasoning IS 'ML prediction reasoning/explanation';
    END IF;
    
    -- Add status column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'status') THEN
        ALTER TABLE tender_records ADD COLUMN status VARCHAR(20) DEFAULT 'pending';
        COMMENT ON COLUMN tender_records.status IS 'Processing status: pending, bid, no-bid, etc.';
    END IF;
    
    -- Add updated_at column for tracking changes
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'tender_records' AND column_name = 'updated_at') THEN
        ALTER TABLE tender_records ADD COLUMN updated_at TIMESTAMP DEFAULT NOW();
        COMMENT ON COLUMN tender_records.updated_at IS 'Last update timestamp';
    END IF;
END $$;

-- Step 2: Convert date columns from TEXT to DATE
-- Handle various date formats including timestamps

-- First, drop NOT NULL constraints to allow conversion
ALTER TABLE tender_records ALTER COLUMN published DROP NOT NULL;
ALTER TABLE tender_records ALTER COLUMN deadline DROP NOT NULL;
ALTER TABLE tender_records ALTER COLUMN awarddate DROP NOT NULL;

-- Convert published column (existing database format)
ALTER TABLE tender_records 
ALTER COLUMN published TYPE TIMESTAMP WITHOUT TIME ZONE 
USING CASE 
    WHEN published ~ '^[A-Za-z]{3} [A-Za-z]{3} \d{1,2} \d{2}:\d{2}:\d{2} [A-Z]{3} \d{4}$' THEN
        TO_TIMESTAMP(published, 'Dy Mon DD HH24:MI:SS TZ YYYY')
    WHEN published = '' OR published IS NULL THEN NULL
    ELSE NULL
END;

-- Convert deadline column
ALTER TABLE tender_records 
ALTER COLUMN deadline TYPE TIMESTAMP WITHOUT TIME ZONE 
USING CASE 
    -- Handle existing database format: "Fri Jun 13 12:54:34 IST 2025"
    WHEN deadline ~ '^[A-Za-z]{3} [A-Za-z]{3} \d{1,2} \d{2}:\d{2}:\d{2} [A-Z]{3} \d{4}$' THEN
        TO_TIMESTAMP(deadline, 'Dy Mon DD HH24:MI:SS TZ YYYY')
    WHEN deadline = '' OR deadline IS NULL THEN NULL
    ELSE NULL
END;

-- Convert awarddate column
ALTER TABLE tender_records 
ALTER COLUMN awarddate TYPE DATE 
USING CASE 
    -- Handle existing database format: "Fri Jun 13 12:54:34 IST 2025"
    WHEN awarddate ~ '^[A-Za-z]{3} [A-Za-z]{3} \d{1,2} \d{2}:\d{2}:\d{2} [A-Z]{3} \d{4}$' THEN
        TO_DATE(awarddate, 'Dy Mon DD HH24:MI:SS TZ YYYY')
    WHEN awarddate = '' OR awarddate IS NULL THEN NULL
    ELSE NULL
END;

-- Step 3: Convert value column from TEXT to DECIMAL
-- Remove currency symbols and commas during conversion
ALTER TABLE tender_records 
ALTER COLUMN value TYPE DECIMAL(15,2) 
USING CASE 
    WHEN value = '' OR value IS NULL THEN NULL
    WHEN value ~ '^€?[\d,]+\.?\d*$' THEN
        CAST(REPLACE(REPLACE(value, '€', ''), ',', '') AS DECIMAL(15,2))
    WHEN value ~ '^£?[\d,]+\.?\d*$' THEN
        CAST(REPLACE(REPLACE(value, '£', ''), ',', '') AS DECIMAL(15,2))
    WHEN value ~ '^[\d,]+\.?\d*$' THEN
        CAST(REPLACE(value, ',', '') AS DECIMAL(15,2))
    ELSE NULL
END;

-- Step 4: Create indexes for better ML performance
CREATE INDEX IF NOT EXISTS idx_tender_published ON tender_records(published);
CREATE INDEX IF NOT EXISTS idx_tender_deadline ON tender_records(deadline);
CREATE INDEX IF NOT EXISTS idx_tender_status ON tender_records(status);
CREATE INDEX IF NOT EXISTS idx_tender_value ON tender_records(value);
CREATE INDEX IF NOT EXISTS idx_tender_ca ON tender_records(ca);
CREATE INDEX IF NOT EXISTS idx_tender_bid ON tender_records(bid);
CREATE INDEX IF NOT EXISTS idx_tender_ml_processed ON tender_records(ml_processed);
CREATE INDEX IF NOT EXISTS idx_tender_ml_bid ON tender_records(ml_bid);
CREATE INDEX IF NOT EXISTS idx_tender_updated_at ON tender_records(updated_at);

-- Step 5: Verify the migration
SELECT 
    COUNT(*) as total_records,
    COUNT(published) as published_dates,
    COUNT(deadline) as deadline_dates,
    COUNT(awarddate) as award_dates,
    COUNT(value) as valid_values,
    COUNT(bid) as labelled_records,
    COUNT(ml_processed) as ml_processed_count,
    COUNT(ml_bid) as ml_predicted_count,
    AVG(value) as avg_value,
    MAX(value) as max_value,
    COUNT(CASE WHEN bid = TRUE THEN 1 END) as positive_labels,
    COUNT(CASE WHEN bid = FALSE THEN 1 END) as negative_labels,
    COUNT(CASE WHEN ml_bid = TRUE THEN 1 END) as ml_positive_predictions,
    COUNT(CASE WHEN ml_bid = FALSE THEN 1 END) as ml_negative_predictions,
    COUNT(CASE WHEN ml_processed = TRUE THEN 1 END) as ml_processed_records,
    COUNT(CASE WHEN status = 'bid' THEN 1 END) as bid_status_count,
    COUNT(CASE WHEN status = 'no-bid' THEN 1 END) as no_bid_status_count,
    COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending_status_count
FROM tender_records;

-- Step 5b: Debug query to see original date formats (run BEFORE migration)
-- Uncomment this to see what date formats are actually in your data:
/*
SELECT DISTINCT 
    published,
    deadline,
    awarddate,
    value
FROM tender_records 
WHERE published IS NOT NULL 
LIMIT 10;
*/

-- Step 6: Show data type verification
SELECT 
    column_name, 
    data_type, 
    is_nullable,
    column_default
FROM information_schema.columns 
WHERE table_name = 'tender_records' 
ORDER BY ordinal_position; 
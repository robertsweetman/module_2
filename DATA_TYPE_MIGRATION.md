# Data Type Migration for Irish Tender Database

## Overview
This document outlines the migration from string-based data storage to proper typed data for ML processing of Irish government tender records.

## Changes Made

### 1. Database Schema Updates

**Old Schema (All TEXT)**:
- `published: TEXT`
- `deadline: TEXT`
- `awarddate: TEXT`
- `value: TEXT`

**New Schema (Proper Types)**:
- `published: TIMESTAMP`
- `deadline: DATE`
- `awarddate: DATE`
- `value: DECIMAL(15,2)`
- `bid: INTEGER` (NEW - for ML labelling 1=bid, 0=no bid)

### 2. Rust Code Changes

#### Dependencies Added to `Cargo.toml`:
```toml
chrono = { version = "0.4", features = ["serde"] }
bigdecimal = { version = "0.4", features = ["serde"] }
regex = "1.10"
sqlx = { version = "0.8.6", features = ["postgres", "runtime-tokio-native-tls", "chrono", "bigdecimal"] }
```

#### New Struct Definitions:
```rust
// Raw data from web scraping (all strings)
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecordRaw {
    title: String,
    resource_id: String,
    ca: String,
    info: String,
    published: String,
    deadline: String,
    procedure: String,
    status: String,
    pdf_url: String,
    awarddate: String,
    value: String,
    cycle: String,
}

// Properly typed data for database storage
#[derive(Debug, Serialize, Deserialize, Clone)]
struct TenderRecord {
    title: String,
    resource_id: String,
    ca: String,
    info: String,
    published: Option<NaiveDate>,
    deadline: Option<NaiveDate>,
    procedure: String,
    status: String,
    pdf_url: String,
    awarddate: Option<NaiveDate>,
    value: Option<BigDecimal>,
    cycle: String,
    bid: Option<bool>, // ML label for supervised learning
}
```

#### Parsing Functions:
- `parse_irish_date()`: Handles various Irish date formats (DD/MM/YYYY, DD-MM-YYYY, etc.)
- `parse_tender_value()`: Parses monetary values with currency symbols and commas

#### Conversion Implementation:
```rust
impl From<TenderRecordRaw> for TenderRecord {
    fn from(raw: TenderRecordRaw) -> Self {
        // Automatically converts raw string data to proper types
    }
}
```

## Migration Steps

### Step 1: Database Migration (Run in pgAdmin4)
Execute the `database_migration.sql` script which:
1. Adds the BID column for ML labelling (if not exists)
2. Converts existing columns to proper types using ALTER TABLE (safer than DROP)
3. Handles data type conversions with USING clauses
4. Creates performance indexes

### Step 2: Code Deployment
Deploy the updated Rust code with:
- New dependencies
- Updated struct definitions
- Type parsing logic
- Database schema updates

## Benefits for ML Processing

### 1. **Date Analysis**
- Proper DATE types enable temporal analysis
- Can calculate days between dates
- Easy filtering by date ranges
- Statistical analysis of timing patterns

### 2. **Value Analysis**
- DECIMAL type enables proper numerical analysis
- Can perform mathematical operations (sum, average, etc.)
- Range filtering and statistical analysis
- Currency normalization

### 3. **Performance**
- Proper indexes on typed columns
- Faster queries for ML feature extraction
- Reduced memory usage for numerical operations

### 4. **Data Quality**
- Type validation during ingestion
- NULL handling for missing data
- Consistent data formats

## Data Parsing Examples

### Date Formats Supported:
- `25/12/2024` → `2024-12-25`
- `25-12-2024` → `2024-12-25`
- `25/12/24` → `2024-12-25`
- Empty/invalid → `NULL`

### Value Formats Supported:
- `€1,234,567.89` → `1234567.89`
- `£50,000` → `50000.00`
- `1,500.50` → `1500.50`
- Empty/invalid → `NULL`
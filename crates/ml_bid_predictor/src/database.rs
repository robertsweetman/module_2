use sqlx::{PgPool, Row};
use anyhow::{Result, Context};
use tracing::{info, warn};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL environment variable not set")?;
        
        let pool = PgPool::connect(&database_url)
            .await
            .context("Failed to connect to database")?;
        
        let db = Database { pool };
        
        // Ensure ml_processed column exists
        db.ensure_ml_processed_column().await?;
        
        Ok(db)
    }

    async fn ensure_ml_processed_column(&self) -> Result<()> {
                // Check if ml_processed column exists
        let column_exists_query = r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.columns 
                WHERE table_name = 'tender_records' 
                AND column_name = 'ml_processed'
            )
        "#;
        
        let column_exists: bool = sqlx::query_scalar::<_, bool>(column_exists_query)
            .fetch_one(&self.pool)
            .await?;

        if !column_exists {
            info!("Adding ml_processed column to tender_records table");
            let add_column_query = r#"
                ALTER TABLE tender_records 
                ADD COLUMN ml_processed BOOLEAN DEFAULT FALSE,
                ADD COLUMN ml_confidence DECIMAL(5,4),
                ADD COLUMN ml_reasoning TEXT,
                ADD COLUMN status VARCHAR(20) DEFAULT 'pending'
            "#;
            
            sqlx::query(add_column_query)
                .execute(&self.pool)
                .await
                .context("Failed to add ml_processed column")?;
            
            info!("Successfully added ml_processed column");
        } else {
            info!("ml_processed column already exists");
        }

        Ok(())
    }

    pub async fn update_ml_processed_status(&self, resource_id: i64, status: &str) -> Result<()> {
        let query = r#"
            UPDATE tender_records 
            SET ml_processed = TRUE,
                status = $2
            WHERE resource_id = $1
        "#;
        
        let rows_affected = sqlx::query(query)
            .bind(resource_id)
            .bind(status)
            .execute(&self.pool)
            .await
            .context("Failed to update ml_processed status")?
            .rows_affected();

        if rows_affected == 0 {
            warn!("No tender found with resource_id: {}", resource_id);
        } else {
            info!("Updated ml_processed status to '{}' for tender: {}", status, resource_id);
        }

        Ok(())
    }

    pub async fn update_ml_prediction_results(
        &self, 
        resource_id: i64, 
        ml_bid: bool, 
        ml_confidence: f64,
        ml_reasoning: &str,
        status: &str
    ) -> Result<()> {
        let query = r#"
            UPDATE tender_records 
            SET ml_bid = $1,
                ml_confidence = $2,
                ml_reasoning = $3,
                ml_processed = TRUE,
                status = $4,
                updated_at = NOW()
            WHERE resource_id = $5
        "#;
        
        let rows_affected = sqlx::query(query)
            .bind(ml_bid)
            .bind(ml_confidence)
            .bind(ml_reasoning)
            .bind(status)
            .bind(resource_id)
            .execute(&self.pool)
            .await
            .with_context(|| format!("Failed to update ML prediction results for resource_id: {}", resource_id))?
            .rows_affected();

        if rows_affected == 0 {
            warn!("No tender found with resource_id: {}", resource_id);
        } else {
            info!("Updated ML prediction results for tender: {} (bid: {}, confidence: {:.3})", 
                  resource_id, ml_bid, ml_confidence);
        }

        Ok(())
    }

    pub async fn get_tender_by_resource_id(&self, resource_id: i64) -> Result<Option<crate::types::TenderRecord>> {
        let query = r#"
            SELECT 
                resource_id,
                title,
                ca,
                procedure,
                pdf_text,
                codes_count,
                published_date,
                deadline,
                estimated_value,
                description,
                pdf_url,
                source,
                bid,
                ml_bid,
                ml_confidence,
                ml_reasoning
            FROM tender_records 
            WHERE resource_id = $1
        "#;
        
        let row = sqlx::query(query)
            .bind(resource_id)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to fetch tender by resource_id")?;

        if let Some(row) = row {
            Ok(Some(crate::types::TenderRecord {
                resource_id: row.get("resource_id"),
                title: row.get("title"),
                contracting_authority: row.get("ca"),
                info: row.get("description"),
                status: row.get("status"),
                procedure: row.get("procedure"),
                pdf_content: row.get("pdf_text"),
                detected_codes: None, // Not stored in the tender table, comes from pipeline
                codes_count: row.get("codes_count"),
                published: row.get("published_date"),
                deadline: row.get("deadline"),
                value: row.get("estimated_value"),
                // Code fields should be determined from codes.txt processing, not database
                pdf_url: row.get("pdf_url"),
                awarddate: row.get("awarddate"),
                cycle: row.get("cycle"),
                processing_stage: row.get("processing_stage"),
                bid: row.get("bid"),
                ml_bid: row.get("ml_bid"),
                ml_confidence: row.get("ml_confidence"),
                ml_reasoning: row.get("ml_reasoning"),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

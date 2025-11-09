use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use tracing::{info, warn};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL environment variable not set")?;

        let pool = PgPool::connect(&database_url)
            .await
            .context("Failed to connect to database")?;

        let db = Database { pool };

        // Ensure ml_processed column exists
        db.ensure_ml_processed_column().await?;

        Ok(db)
    }

    async fn ensure_ml_processed_column(&self) -> Result<()> {
        info!("Ensuring ML columns exist in tender_records table");

        // Add all ML columns using IF NOT EXISTS to be idempotent
        // This will succeed whether columns exist or not
        let migrations = vec![
            (
                "ml_processed",
                "ALTER TABLE tender_records ADD COLUMN IF NOT EXISTS ml_processed BOOLEAN DEFAULT FALSE",
            ),
            (
                "ml_bid",
                "ALTER TABLE tender_records ADD COLUMN IF NOT EXISTS ml_bid BOOLEAN",
            ),
            (
                "ml_confidence",
                "ALTER TABLE tender_records ADD COLUMN IF NOT EXISTS ml_confidence DECIMAL(5,4)",
            ),
            (
                "ml_reasoning",
                "ALTER TABLE tender_records ADD COLUMN IF NOT EXISTS ml_reasoning TEXT",
            ),
            (
                "ml_status",
                "ALTER TABLE tender_records ADD COLUMN IF NOT EXISTS ml_status VARCHAR(20) DEFAULT 'pending'",
            ),
        ];

        for (column_name, query) in migrations {
            match sqlx::query(query).execute(&self.pool).await {
                Ok(_) => {
                    info!("✓ Ensured {} column exists", column_name);
                }
                Err(e) => {
                    warn!(
                        "Failed to add {} column (might already exist): {}",
                        column_name, e
                    );
                    // Don't fail - column might already exist with different syntax
                }
            }
        }

        info!("✅ ML columns migration complete");
        Ok(())
    }

    pub async fn update_ml_processed_status(
        &self,
        resource_id: i64,
        ml_status: &str,
    ) -> Result<()> {
        let query = r#"
            UPDATE tender_records
            SET ml_processed = TRUE,
                ml_status = $2
            WHERE resource_id = $1
        "#;

        let rows_affected = sqlx::query(query)
            .bind(resource_id)
            .bind(ml_status)
            .execute(&self.pool)
            .await
            .context("Failed to update ml_processed status")?
            .rows_affected();

        if rows_affected == 0 {
            warn!("No tender found with resource_id: {}", resource_id);
        } else {
            info!(
                "Updated ml_status to '{}' for tender: {}",
                ml_status, resource_id
            );
        }

        Ok(())
    }

    pub async fn update_ml_prediction_results(
        &self,
        resource_id: i64,
        ml_bid: bool,
        ml_confidence: f64,
        ml_reasoning: &str,
        ml_status: &str,
    ) -> Result<()> {
        let query = r#"
            UPDATE tender_records
            SET ml_bid = $1,
                ml_confidence = $2,
                ml_reasoning = $3,
                ml_status = $4,
                ml_processed = TRUE,
                updated_at = NOW()
            WHERE resource_id = $5
        "#;

        let rows_affected = sqlx::query(query)
            .bind(ml_bid)
            .bind(ml_confidence)
            .bind(ml_reasoning)
            .bind(ml_status)
            .bind(resource_id)
            .execute(&self.pool)
            .await
            .with_context(|| {
                format!(
                    "Failed to update ML prediction results for resource_id: {}",
                    resource_id
                )
            })?
            .rows_affected();

        if rows_affected == 0 {
            warn!("No tender found with resource_id: {}", resource_id);
        } else {
            info!(
                "Updated ML prediction results for tender: {} (bid: {}, confidence: {:.3})",
                resource_id, ml_bid, ml_confidence
            );
        }

        Ok(())
    }

    pub async fn get_tender_by_resource_id(
        &self,
        resource_id: i64,
    ) -> Result<Option<crate::types::TenderRecord>> {
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

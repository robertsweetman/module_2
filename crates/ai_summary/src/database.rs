use crate::types::{PdfContent, TenderRecord, Config};
use sqlx::{Pool, Postgres, Row};
use anyhow::Result;
use tracing::{info, debug, warn};
use chrono::{DateTime, Utc};

/// Database operations for AI summary processing
pub struct Database {
    pool: Pool<Postgres>,
}

impl Database {
    /// Create new database connection
    pub async fn new(config: &Config) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;
        
        info!("✅ Database connection established");
        Ok(Self { pool })
    }
    
    /// Get complete PDF content from pdf_content table
    pub async fn get_pdf_content(&self, resource_id: i64) -> Result<Option<PdfContent>> {
        debug!("🔍 Fetching PDF content for resource_id: {}", resource_id);
        
        let row = sqlx::query(
            r#"
            SELECT 
                resource_id,
                pdf_text,
                detected_codes,
                codes_count,
                extraction_timestamp
            FROM pdf_content 
            WHERE resource_id = $1
            "#
        )
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let detected_codes: Vec<String> = row.get::<Option<Vec<String>>, _>("detected_codes")
                .unwrap_or_default();
            
            let pdf_content = PdfContent {
                resource_id: row.get("resource_id"),
                pdf_text: row.get("pdf_text"),
                detected_codes,
                codes_count: row.get::<Option<i32>, _>("codes_count").unwrap_or(0),
                extraction_timestamp: row.get::<chrono::NaiveDateTime, _>("extraction_timestamp")
                    .and_utc(),
            };
            
            info!("✅ Found PDF content for resource_id: {}, text length: {}", 
                  resource_id, pdf_content.pdf_text.len());
            Ok(Some(pdf_content))
        } else {
            warn!("⚠️ No PDF content found for resource_id: {}", resource_id);
            Ok(None)
        }
    }
    
    /// Get complete tender record from main tender table
    pub async fn get_tender_record(&self, resource_id: i64) -> Result<Option<TenderRecord>> {
        debug!("🔍 Fetching tender record for resource_id: {}", resource_id);
        
        let row = sqlx::query(
            r#"
            SELECT 
                resource_id,
                title,
                ca as contracting_authority,
                description as info,
                published_date as published,
                deadline,
                procedure,
                status,
                pdf_url,
                awarddate,
                estimated_value as value,
                cycle,
                bid,
                processing_stage
            FROM tenders 
            WHERE resource_id = $1
            "#
        )
        .bind(resource_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            let tender = TenderRecord {
                resource_id: row.get("resource_id"),
                title: row.get("title"),
                contracting_authority: row.get("contracting_authority"),
                info: row.get("info"),
                published: row.get("published"),
                deadline: row.get("deadline"),
                procedure: row.get("procedure"),
                status: row.get("status"),
                pdf_url: row.get("pdf_url"),
                awarddate: row.get("awarddate"),
                value: row.get("value"),
                cycle: row.get("cycle"),
                bid: row.get("bid"),
                pdf_content: None, // Will be populated separately if needed
                detected_codes: None, // Will be populated from pdf_content table
                codes_count: None, // Will be populated from pdf_content table
                processing_stage: row.get("processing_stage"),
            };
            
            info!("✅ Found tender record for resource_id: {}", resource_id);
            Ok(Some(tender))
        } else {
            warn!("⚠️ No tender record found for resource_id: {}", resource_id);
            Ok(None)
        }
    }
    
    /// Store AI summary result
    pub async fn store_ai_summary(&self, summary: &crate::types::AISummaryResult) -> Result<()> {
        info!("💾 Storing AI summary for resource_id: {}", summary.resource_id);
        
        // Create ai_summaries table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS ai_summaries (
                resource_id BIGINT PRIMARY KEY,
                summary_type TEXT NOT NULL,
                ai_summary TEXT NOT NULL,
                key_points JSONB NOT NULL,
                recommendation TEXT NOT NULL,
                confidence_assessment TEXT NOT NULL,
                processing_notes JSONB NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(&self.pool)
        .await?;
        
        // Insert or update summary
        sqlx::query(
            r#"
            INSERT INTO ai_summaries 
            (resource_id, summary_type, ai_summary, key_points, recommendation, 
             confidence_assessment, processing_notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (resource_id) 
            DO UPDATE SET 
                summary_type = EXCLUDED.summary_type,
                ai_summary = EXCLUDED.ai_summary,
                key_points = EXCLUDED.key_points,
                recommendation = EXCLUDED.recommendation,
                confidence_assessment = EXCLUDED.confidence_assessment,
                processing_notes = EXCLUDED.processing_notes,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(summary.resource_id)
        .bind(&summary.summary_type)
        .bind(&summary.ai_summary)
        .bind(serde_json::to_value(&summary.key_points)?)
        .bind(&summary.recommendation)
        .bind(&summary.confidence_assessment)
        .bind(serde_json::to_value(&summary.processing_notes)?)
        .bind(summary.created_at)
        .execute(&self.pool)
        .await?;
        
        info!("✅ Stored AI summary for resource_id: {}", summary.resource_id);
        Ok(())
    }
}

use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // Get database URL from environment variable
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    println!("Attempting to connect to database...");
    
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    
    println!("Successfully connected to database!");
    
    // Test a simple query
    let result = sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await?;
    
    println!("Query result: {:?}", result);
    
    Ok(())
} 
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::env;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    // Ensure the 'bid' column exists
    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM information_schema.columns
                WHERE table_name='tender_records' AND column_name='bid'
            ) THEN
                ALTER TABLE tender_records ADD COLUMN bid BOOLEAN;
            END IF;
        END
        $$;
        "#
    )
    .execute(&pool)
    .await?;

    loop {
        // Fetch a random unlabeled record
        let row = sqlx::query(
            r#"
            SELECT id, title, ca, info
            FROM tender_records
            WHERE bid IS NULL
            ORDER BY random()
            LIMIT 1
            "#
        )
        .fetch_optional(&pool)
        .await?;

        if let Some(r) = row {
            let id: i32 = r.get("id");
            let title: String = r.get("title");
            let ca: String = r.get("ca");
            let info: String = r.get("info");

            println!("\nTitle: {}", title);
            println!("CA: {}", ca);
            println!("Info: {}", info);

            print!("Bid? (y/n/quit): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            if input == "quit" {
                break;
            } else if input == "y" || input == "yes" {
                sqlx::query(
                    "UPDATE tender_records SET bid = $1 WHERE id = $2"
                )
                .bind(true)
                .bind(id)
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = true", id);
            } else if input == "n" || input == "no" {
                sqlx::query(
                    "UPDATE tender_records SET bid = $1 WHERE id = $2"
                )
                .bind(false)
                .bind(id)
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = false", id);
            } else {
                println!("Please enter 'y', 'n', or 'quit'.");
            }
        } else {
            println!("No more unlabeled records!");
            break;
        }
    }

    Ok(())
} 
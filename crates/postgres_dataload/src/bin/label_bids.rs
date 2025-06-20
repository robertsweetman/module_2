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

    // Ensure the 'bid' column exists as INTEGER (0=no, 1=yes, NULL=unlabeled)
    sqlx::query(
        r#"
        DO $$
        BEGIN
            /* Case 1: column does not exist -> create as INTEGER */
            IF NOT EXISTS (
                SELECT 1
                FROM information_schema.columns
                WHERE table_name = 'tender_records' AND column_name = 'bid'
            ) THEN
                ALTER TABLE tender_records ADD COLUMN bid INTEGER;

            /* Case 2: column exists but is BOOLEAN -> convert to INTEGER */
            ELSIF EXISTS (
                SELECT 1
                FROM information_schema.columns
                WHERE table_name = 'tender_records'
                  AND column_name = 'bid'
                  AND data_type = 'boolean'
            ) THEN
                -- Convert TRUE/FALSE to 1/0 while changing type
                ALTER TABLE tender_records
                ALTER COLUMN bid DROP DEFAULT,
                ALTER COLUMN bid TYPE INTEGER USING (CASE WHEN bid IS TRUE THEN 1 WHEN bid IS FALSE THEN 0 ELSE NULL END);
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
                .bind(1) // 1 = yes, is a bid
                .bind(id)
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = 1 (yes)", id);
            } else if input == "n" || input == "no" {
                sqlx::query(
                    "UPDATE tender_records SET bid = $1 WHERE id = $2"
                )
                .bind(0) // 0 = no, not a bid
                .bind(id)
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = 0 (no)", id);
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
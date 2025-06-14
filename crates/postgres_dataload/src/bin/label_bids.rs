use sqlx::postgres::PgPoolOptions;
use std::env;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    loop {
        // Fetch a random unlabeled record
        let record = sqlx::query!(
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

        if let Some(r) = record {
            println!("\nTitle: {}", r.title);
            println!("CA: {}", r.ca);
            println!("Info: {}", r.info);

            print!("Bid? (y/n/quit): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            if input == "quit" {
                break;
            } else if input == "y" || input == "yes" {
                sqlx::query!(
                    "UPDATE tender_records SET bid = $1 WHERE id = $2",
                    true,
                    r.id
                )
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = true", r.id);
            } else if input == "n" || input == "no" {
                sqlx::query!(
                    "UPDATE tender_records SET bid = $1 WHERE id = $2",
                    false,
                    r.id
                )
                .execute(&pool)
                .await?;
                println!("Updated record {} with bid = false", r.id);
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
use core_war_backend::auth::password::hash_password;
use sqlx::postgres::PgPoolOptions;
use std::env;
use uuid::Uuid;

const IMP_SOURCE: &str = include_str!("../../../engine/tests/warriors/imp.red");
const DWARF_SOURCE: &str = include_str!("../../../engine/tests/warriors/dwarf.red");

struct SeedUser {
    username: &'static str,
    email: &'static str,
    warrior_name: &'static str,
    warrior_source: &'static str,
}

const SEED_USERS: &[SeedUser] = &[
    SeedUser {
        username: "_test_red",
        email: "_test_red@local.test",
        warrior_name: "Imp",
        warrior_source: IMP_SOURCE,
    },
    SeedUser {
        username: "_test_blue",
        email: "_test_blue@local.test",
        warrior_name: "Dwarf",
        warrior_source: DWARF_SOURCE,
    },
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url =
        env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set (check backend/.env)")?;
    let password = env::var("TEST_USER_PASSWORD")
        .map_err(|_| "TEST_USER_PASSWORD must be set (check backend/.env)")?;

    if password.len() < 8 {
        return Err("TEST_USER_PASSWORD must be at least 8 characters".into());
    }

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let password_hash = hash_password(&password).map_err(|e| format!("hashing failed: {e:?}"))?;

    for seed in SEED_USERS {
        let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE username = $1")
            .bind(seed.username)
            .fetch_optional(&pool)
            .await?;

        let user_id = if let Some((id,)) = existing {
            println!(
                "user {} already exists ({id}), skipping user insert",
                seed.username
            );
            id
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO users (username, email, password_hash) \
                 VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(seed.username)
            .bind(seed.email)
            .bind(&password_hash)
            .fetch_one(&pool)
            .await?;
            println!("created user {} ({id})", seed.username);
            id
        };

        let warrior_exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM warriors WHERE user_id = $1 AND name = $2")
                .bind(user_id)
                .bind(seed.warrior_name)
                .fetch_optional(&pool)
                .await?;

        if let Some((id,)) = warrior_exists {
            println!(
                "  warrior {} already exists ({id}) for {}, skipping",
                seed.warrior_name, seed.username
            );
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO warriors (user_id, name, source) \
                 VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(user_id)
            .bind(seed.warrior_name)
            .bind(seed.warrior_source)
            .fetch_one(&pool)
            .await?;
            println!(
                "  created warrior {} ({id}) for {}",
                seed.warrior_name, seed.username
            );
        }
    }

    println!("seed complete");
    Ok(())
}

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct MatchRecord {
    pub id: Uuid,
    pub red_warrior_id: Uuid,
    pub blue_warrior_id: Uuid,
    pub red_user_id: Uuid,
    pub blue_user_id: Uuid,
    pub core_size: i32,
    pub max_steps: i32,
    pub result: String,
    pub steps_taken: i32,
    pub created_at: DateTime<Utc>,
}

use crate::auth::middleware::AuthUser;
use crate::errors::AppError;
use crate::models::match_record::MatchRecord;
use crate::models::warrior::Warrior;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use core_war_engine::{parse_warrior, MatchResult, MatchState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CORE_SIZE: usize = 8000;
const MAX_STEPS: u64 = 80_000;

#[derive(Deserialize)]
pub struct SubmitMatchRequest {
    pub red_warrior_id: Uuid,
    pub blue_warrior_id: Uuid,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Serialize)]
pub struct MatchListResponse {
    pub matches: Vec<MatchRecord>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

const MAX_PAGE_SIZE: i64 = 100;
const DEFAULT_PAGE_SIZE: i64 = 50;

fn run_battle(red_source: &str, blue_source: &str) -> Result<(String, i32), AppError> {
    let red = parse_warrior(red_source)
        .map_err(|e| AppError::BadRequest(format!("Red warrior parse error: {e}")))?;
    let blue = parse_warrior(blue_source)
        .map_err(|e| AppError::BadRequest(format!("Blue warrior parse error: {e}")))?;

    let mut m = MatchState::new(CORE_SIZE, MAX_STEPS);
    m.load_warrior(0, &red, 0);
    m.load_warrior(1, &blue, CORE_SIZE / 2);

    // Issue #87: engine's step() keeps running the survivor after a Victory,
    // so this loop would otherwise always pin steps_taken at MAX_STEPS.
    while m.step() {
        if !matches!(m.result(), MatchResult::Ongoing) {
            break;
        }
    }

    let result_str = match m.result() {
        MatchResult::Victory { winner_id: 0 } => "red_win",
        MatchResult::Victory { .. } => "blue_win",
        MatchResult::Tie => "tie",
        MatchResult::AllDead => "all_dead",
        MatchResult::Ongoing => unreachable!(),
    };

    Ok((result_str.to_string(), m.steps() as i32))
}

pub async fn submit(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<SubmitMatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let red_warrior = sqlx::query_as::<_, Warrior>(
        "SELECT id, user_id, name, source, created_at, updated_at \
         FROM warriors WHERE id = $1",
    )
    .bind(body.red_warrior_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Red warrior not found".into()))?;

    let blue_warrior = sqlx::query_as::<_, Warrior>(
        "SELECT id, user_id, name, source, created_at, updated_at \
         FROM warriors WHERE id = $1",
    )
    .bind(body.blue_warrior_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Blue warrior not found".into()))?;

    if red_warrior.user_id != user.user_id && blue_warrior.user_id != user.user_id {
        return Err(AppError::Forbidden(
            "You must own at least one of the warriors".into(),
        ));
    }

    let (result, steps_taken) = {
        let red_src = red_warrior.source.clone();
        let blue_src = blue_warrior.source.clone();
        tokio::task::spawn_blocking(move || run_battle(&red_src, &blue_src))
            .await
            .map_err(|e| AppError::Internal(format!("Battle task failed: {e}")))?
    }?;

    let record = sqlx::query_as::<_, MatchRecord>(
        "INSERT INTO matches \
           (red_warrior_id, blue_warrior_id, red_user_id, blue_user_id, \
            core_size, max_steps, result, steps_taken) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING *",
    )
    .bind(red_warrior.id)
    .bind(blue_warrior.id)
    .bind(red_warrior.user_id)
    .bind(blue_warrior.user_id)
    .bind(CORE_SIZE as i32)
    .bind(MAX_STEPS as i32)
    .bind(&result)
    .bind(steps_taken)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(record)))
}

pub async fn get(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<MatchRecord>, AppError> {
    let record = sqlx::query_as::<_, MatchRecord>("SELECT * FROM matches WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found".into()))?;

    Ok(Json(record))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<MatchListResponse>, AppError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query
        .per_page
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * per_page;

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM matches WHERE red_user_id = $1 OR blue_user_id = $1",
    )
    .bind(user.user_id)
    .fetch_one(&state.db)
    .await?;

    let matches = sqlx::query_as::<_, MatchRecord>(
        "SELECT * FROM matches \
         WHERE red_user_id = $1 OR blue_user_id = $1 \
         ORDER BY created_at DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(user.user_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(MatchListResponse {
        matches,
        total,
        page,
        per_page,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_battle_imp_vs_imp_ties() {
        let imp = "MOV.I $0, $1";
        let (result, steps) = run_battle(imp, imp).unwrap();
        assert_eq!(result, "tie");
        assert_eq!(steps, MAX_STEPS as i32);
    }

    #[test]
    fn run_battle_decisive_match_stops_short_of_max_steps() {
        // Issue #87: without the non-Ongoing short-circuit this test would
        // silently pass at steps == MAX_STEPS because the engine keeps
        // stepping the survivor. Use Dwarf-vs-mice-lite (rather than
        // Dwarf-vs-Imp, which is deceptively a tie at 8000/80k) so the
        // decision reliably lands well before MAX_STEPS and any regression
        // that lets the loop continue past Victory fails loudly here.
        let dwarf = ";name Dwarf\n  ORG start\nstart ADD.AB #4, bomb\n  MOV.I bomb, @bomb\n  JMP start\nbomb DAT.F #0, #0";
        let mice_lite = ";name Mice-Lite\n        ORG    loop\ncounter DAT.F  #0, #3\ndest    DAT.F  #0, #8\nimp     MOV.I  $0, $1\nloop    MOV.I  imp, <dest\n        DJN.B  loop, counter\nlanding DAT.F  #0, #0\n        END";
        let (result, steps) = run_battle(dwarf, mice_lite).unwrap();
        assert_eq!(result, "red_win");
        assert!(
            steps < 50,
            "mice-lite self-terminates quickly; got {steps} steps"
        );
    }

    #[test]
    fn run_battle_invalid_source() {
        let result = run_battle("INVALID NONSENSE", "MOV.I $0, $1");
        assert!(result.is_err());
    }
}

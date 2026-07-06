use crate::leaderboard::elo::{rating_change, tie_change};
use core_war_engine::{parse_warrior, MatchResult, MatchState};
use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

/// Everything the caller needs to render the result screen: canonical outcome
/// plus each side's rating before and after the match. Returned by
/// [`execute_and_persist_match`] once the transaction has committed, so an `Ok`
/// value implies both the match row and both rating updates are durable.
#[derive(Debug, Clone)]
pub struct MatchOutcome {
    pub match_id: Uuid,
    pub result: String,
    pub steps_taken: i32,
    pub red_rating_before: i32,
    pub red_rating_after: i32,
    pub blue_rating_before: i32,
    pub blue_rating_after: i32,
}

#[derive(Debug)]
pub enum MatchExecuteError {
    ParseRed(String),
    ParseBlue(String),
    Db(sqlx::Error),
    Panic(String),
}

impl fmt::Display for MatchExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseRed(e) => write!(f, "Red warrior parse error: {e}"),
            Self::ParseBlue(e) => write!(f, "Blue warrior parse error: {e}"),
            Self::Db(e) => write!(f, "DB error: {e}"),
            Self::Panic(e) => write!(f, "Battle task panic: {e}"),
        }
    }
}

impl std::error::Error for MatchExecuteError {}

impl From<sqlx::Error> for MatchExecuteError {
    fn from(err: sqlx::Error) -> Self {
        Self::Db(err)
    }
}

/// Runs a battle to completion and persists the outcome plus rating changes.
///
/// The match row insert and both `users.rating` updates happen inside a single
/// Postgres transaction — either all three land or none do, so a caller that
/// sees `Ok(_)` knows the leaderboard and the match history agree.
///
/// `all_dead` (mutual destruction) is treated the same as `tie` for rating
/// purposes — neither warrior outplayed the other, so the shift is toward the
/// lower-rated player rather than either being credited with a win.
#[allow(clippy::too_many_arguments)]
pub async fn execute_and_persist_match(
    db: &PgPool,
    red_user_id: Uuid,
    blue_user_id: Uuid,
    red_warrior_id: Uuid,
    blue_warrior_id: Uuid,
    red_source: &str,
    blue_source: &str,
    core_size: usize,
    max_steps: u64,
) -> Result<MatchOutcome, MatchExecuteError> {
    let red_src = red_source.to_owned();
    let blue_src = blue_source.to_owned();
    let (result_str, steps_taken) =
        tokio::task::spawn_blocking(move || run_battle(&red_src, &blue_src, core_size, max_steps))
            .await
            .map_err(|e| MatchExecuteError::Panic(e.to_string()))??;

    let mut tx = db.begin().await?;

    let red_rating_before: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE id = $1")
        .bind(red_user_id)
        .fetch_one(&mut *tx)
        .await?;
    let blue_rating_before: i32 = sqlx::query_scalar("SELECT rating FROM users WHERE id = $1")
        .bind(blue_user_id)
        .fetch_one(&mut *tx)
        .await?;

    let (red_rating_after, blue_rating_after) = match result_str.as_str() {
        "red_win" => rating_change(red_rating_before, blue_rating_before),
        "blue_win" => {
            // rating_change returns (winner, loser); remap so the tuple stays (red, blue).
            let (new_blue, new_red) = rating_change(blue_rating_before, red_rating_before);
            (new_red, new_blue)
        }
        "tie" | "all_dead" => tie_change(red_rating_before, blue_rating_before),
        other => unreachable!("battle produced unexpected result string: {other}"),
    };

    let match_id: Uuid = sqlx::query_scalar(
        "INSERT INTO matches \
           (red_warrior_id, blue_warrior_id, red_user_id, blue_user_id, \
            core_size, max_steps, result, steps_taken) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id",
    )
    .bind(red_warrior_id)
    .bind(blue_warrior_id)
    .bind(red_user_id)
    .bind(blue_user_id)
    .bind(core_size as i32)
    .bind(max_steps as i32)
    .bind(&result_str)
    .bind(steps_taken)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE users SET rating = $1 WHERE id = $2")
        .bind(red_rating_after)
        .bind(red_user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE users SET rating = $1 WHERE id = $2")
        .bind(blue_rating_after)
        .bind(blue_user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(MatchOutcome {
        match_id,
        result: result_str,
        steps_taken,
        red_rating_before,
        red_rating_after,
        blue_rating_before,
        blue_rating_after,
    })
}

fn run_battle(
    red_source: &str,
    blue_source: &str,
    core_size: usize,
    max_steps: u64,
) -> Result<(String, i32), MatchExecuteError> {
    let red = parse_warrior(red_source).map_err(|e| MatchExecuteError::ParseRed(e.to_string()))?;
    let blue =
        parse_warrior(blue_source).map_err(|e| MatchExecuteError::ParseBlue(e.to_string()))?;

    let mut m = MatchState::new(core_size, max_steps);
    m.load_warrior(0, &red, 0);
    m.load_warrior(1, &blue, core_size / 2);

    // Issue #87: the engine's `step()` keeps executing the surviving warrior
    // after a Victory is decided, so left uncapped this loop always runs to
    // max_steps and pins `steps_taken` at MAX_STEPS. Stop as soon as the
    // result is no longer Ongoing — a genuine tie still runs to the step
    // limit because Imp-vs-Imp stays Ongoing until then.
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
        MatchResult::Ongoing => unreachable!("engine step returned false while Ongoing"),
    };

    Ok((result_str.to_string(), m.steps() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisive_match_stops_short_of_max_steps() {
        // Dwarf-vs-mice-lite (rather than Dwarf-vs-Imp) because Imp is fast
        // enough at CORE_SIZE=8000 to dodge Dwarf's ~13k bombs across an
        // 80k-step budget — the "canonical" pairing is deceptively a tie.
        // Mice-Lite replicates for a few iterations, then falls through to
        // its own DAT landing pad and dies — a reliably decisive match.
        // Kept inline (rather than include_str! from engine/) so the runner
        // unit tests stay self-contained inside this crate.
        let dwarf = ";name Dwarf\n  ORG start\nstart ADD.AB #4, bomb\n  MOV.I bomb, @bomb\n  JMP start\nbomb DAT.F #0, #0";
        let mice_lite = ";name Mice-Lite\n        ORG    loop\ncounter DAT.F  #0, #3\ndest    DAT.F  #0, #8\nimp     MOV.I  $0, $1\nloop    MOV.I  imp, <dest\n        DJN.B  loop, counter\nlanding DAT.F  #0, #0\n        END";
        let (result, steps) = run_battle(dwarf, mice_lite, 8000, 80_000).unwrap();
        assert_eq!(result, "red_win");
        assert!(
            steps < 50,
            "mice-lite self-terminates quickly; got {steps} steps"
        );
    }

    #[test]
    fn imp_vs_imp_still_ties_at_max_steps() {
        let imp = "MOV.I $0, $1";
        let (result, steps) = run_battle(imp, imp, 8000, 80_000).unwrap();
        assert_eq!(result, "tie");
        assert_eq!(steps, 80_000);
    }
}

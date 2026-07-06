use crate::auth::socket::{get_auth, require_auth};
use crate::matchmaking::queue::{QueueEntry, RedisQueue};
use crate::matchmaking::runner::execute_and_persist_match;
use crate::models::warrior::Warrior;
use chrono::Utc;
use core_war_engine::parse_warrior;
use serde::{Deserialize, Serialize};
use socketioxide::extract::{Data, SocketRef};
use socketioxide::{socket::Sid, SocketIo};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{error, info, warn};
use uuid::Uuid;

const CORE_SIZE: usize = 8000;
const MAX_STEPS: u64 = 80_000;
const RED_START: usize = 0;
const BLUE_START: usize = CORE_SIZE / 2;
// Playback rate for the client-side replay. Server runs the battle once, the
// outcome is deterministic, and clients render locally paced from
// `playback_start_time_ms`. Tuning knob for the live viewer UX — higher is
// snappier, lower draws out the moment-to-moment activity.
const STEPS_PER_SEC: u32 = 2000;

#[derive(Deserialize)]
pub struct QueueJoinData {
    pub warrior_id: String,
}

#[derive(Serialize)]
struct QueueJoined {
    position: usize,
    queue_size: usize,
}

#[derive(Serialize, Clone)]
struct MatchFound {
    match_id: String,
    red_username: String,
    blue_username: String,
    red_warrior: String,
    blue_warrior: String,
}

#[derive(Serialize, Clone)]
struct MatchStart {
    match_id: String,
    red_username: String,
    blue_username: String,
    red_warrior_source: String,
    blue_warrior_source: String,
    core_size: usize,
    max_steps: u64,
    red_start: usize,
    blue_start: usize,
    steps_taken: u64,
    result: String,
    playback_start_time_ms: i64,
    steps_per_sec: u32,
}

#[derive(Serialize, Clone)]
struct MatchResultEvent {
    match_id: String,
    result: String,
    steps_taken: u64,
    red_username: String,
    blue_username: String,
}

fn match_room(match_id: &str) -> String {
    format!("match:{match_id}")
}

pub fn register_events(socket: &SocketRef, io: SocketIo, queue: RedisQueue, db: PgPool) {
    let q = queue.clone();
    let pool = db.clone();
    let io_for_join = io.clone();
    socket.on(
        "queue:join",
        move |socket: SocketRef, Data(data): Data<QueueJoinData>| {
            let q = q.clone();
            let pool = pool.clone();
            let io = io_for_join.clone();
            async move {
                let user = match require_auth(&socket) {
                    Some(u) => u,
                    None => return,
                };

                let warrior_id = match Uuid::parse_str(&data.warrior_id) {
                    Ok(id) => id,
                    Err(_) => {
                        let _ = socket.emit(
                            "queue:error",
                            &serde_json::json!({"error": "Invalid warrior ID"}),
                        );
                        return;
                    }
                };

                let warrior = match sqlx::query_as::<_, Warrior>(
                    "SELECT id, user_id, name, source, created_at, updated_at \
                     FROM warriors WHERE id = $1 AND user_id = $2",
                )
                .bind(warrior_id)
                .bind(user.user_id)
                .fetch_optional(&pool)
                .await
                {
                    Ok(Some(w)) => w,
                    Ok(None) => {
                        let _ = socket.emit(
                            "queue:error",
                            &serde_json::json!({"error": "Warrior not found or not owned by you"}),
                        );
                        return;
                    }
                    Err(e) => {
                        error!("DB error loading warrior: {e}");
                        let _ = socket.emit(
                            "queue:error",
                            &serde_json::json!({"error": "Internal error"}),
                        );
                        return;
                    }
                };

                if parse_warrior(&warrior.source).is_err() {
                    let _ = socket.emit(
                        "queue:error",
                        &serde_json::json!({"error": "Warrior source has parse errors"}),
                    );
                    return;
                }

                let entry = QueueEntry {
                    user_id: user.user_id,
                    username: user.username.clone(),
                    warrior_id,
                    socket_id: socket.id.to_string(),
                };

                let match_pair = match q.join(entry).await {
                    Ok(pair) => pair,
                    Err(e) => {
                        error!("Redis error in queue:join: {e}");
                        let _ = socket.emit(
                            "queue:error",
                            &serde_json::json!({"error": "Internal error"}),
                        );
                        return;
                    }
                };

                match match_pair {
                    None => {
                        let pos = q
                            .position(user.user_id)
                            .await
                            .unwrap_or(Some(0))
                            .unwrap_or(0);
                        let size = q.len().await.unwrap_or(0);
                        let _ = socket.emit(
                            "queue:joined",
                            &QueueJoined {
                                position: pos + 1,
                                queue_size: size,
                            },
                        );
                        info!("user {} joined queue (position {})", user.username, pos + 1);
                    }
                    Some((red, blue)) => {
                        info!("match found: {} vs {}", red.username, blue.username);
                        run_matched_battle(io, pool, red, blue).await;
                    }
                }
            }
        },
    );

    let q2 = queue.clone();
    socket.on("queue:leave", move |socket: SocketRef| {
        let q = q2.clone();
        async move {
            let user = match require_auth(&socket) {
                Some(u) => u,
                None => return,
            };

            let removed = match q.leave(user.user_id).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Redis error in queue:leave: {e}");
                    return;
                }
            };

            if removed {
                let _ = socket.emit("queue:left", &serde_json::json!({}));
                info!("user {} left queue", user.username);
            }
        }
    });

    let q3 = queue;
    socket.on_disconnect(move |socket: SocketRef| {
        let q = q3.clone();
        async move {
            if let Some(user) = get_auth(&socket) {
                if let Err(e) = q.leave(user.user_id).await {
                    error!("Redis error on disconnect cleanup: {e}");
                }
            }
        }
    });
}

async fn run_matched_battle(io: SocketIo, db: PgPool, red: QueueEntry, blue: QueueEntry) {
    let red_source =
        match sqlx::query_scalar::<_, String>("SELECT source FROM warriors WHERE id = $1")
            .bind(red.warrior_id)
            .fetch_one(&db)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to load red warrior: {e}");
                return;
            }
        };

    let blue_source =
        match sqlx::query_scalar::<_, String>("SELECT source FROM warriors WHERE id = $1")
            .bind(blue.warrior_id)
            .fetch_one(&db)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to load blue warrior: {e}");
                return;
            }
        };

    let match_id = Uuid::new_v4().to_string();
    let room = match_room(&match_id);

    // Join both participants into a dedicated room for this match. socketioxide
    // does NOT auto-join sockets to a room named after their sid (unlike vanilla
    // Node socket.io), so we must use explicit room names — that's the root cause
    // of the prior `socket.to(sid).emit()` silently dropping events on the other
    // side. Emitting via `io.to(room)` from the SocketIo instance includes all
    // sockets currently in the room (no self-exclusion semantics).
    let red_sid = match Sid::from_str(&red.socket_id) {
        Ok(s) => s,
        Err(e) => {
            error!("invalid red socket_id {}: {e}", red.socket_id);
            return;
        }
    };
    let blue_sid = match Sid::from_str(&blue.socket_id) {
        Ok(s) => s,
        Err(e) => {
            error!("invalid blue socket_id {}: {e}", blue.socket_id);
            return;
        }
    };

    let red_socket = match io.get_socket(red_sid) {
        Some(s) => s,
        None => {
            warn!(
                "red socket {} no longer connected; aborting match",
                red.socket_id
            );
            return;
        }
    };
    let blue_socket = match io.get_socket(blue_sid) {
        Some(s) => s,
        None => {
            warn!(
                "blue socket {} no longer connected; aborting match",
                blue.socket_id
            );
            return;
        }
    };

    let _ = red_socket.join(room.clone());
    let _ = blue_socket.join(room.clone());

    let found_event = MatchFound {
        match_id: match_id.clone(),
        red_username: red.username.clone(),
        blue_username: blue.username.clone(),
        red_warrior: red.warrior_id.to_string(),
        blue_warrior: blue.warrior_id.to_string(),
    };

    let _ = io.to(room.clone()).emit("match:found", &found_event);

    let outcome = match execute_and_persist_match(
        &db,
        red.user_id,
        blue.user_id,
        red.warrior_id,
        blue.warrior_id,
        &red_source,
        &blue_source,
        CORE_SIZE,
        MAX_STEPS,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to execute+persist match: {e}");
            return;
        }
    };

    let result_str = outcome.result.clone();
    let steps_taken = outcome.steps_taken as u64;

    // Emit the full replay payload. Clients use playback_start_time_ms to
    // pace the local wasm replay deterministically; match:result follows
    // immediately as informational confirmation of the canonical outcome.
    // No server-side scheduling — pacing is purely client-side, which makes
    // spectator join-in-progress trivial (compute current step from elapsed
    // time and fast-forward locally).
    let start_event = MatchStart {
        match_id: match_id.clone(),
        red_username: red.username.clone(),
        blue_username: blue.username.clone(),
        red_warrior_source: red_source,
        blue_warrior_source: blue_source,
        core_size: CORE_SIZE,
        max_steps: MAX_STEPS,
        red_start: RED_START,
        blue_start: BLUE_START,
        steps_taken,
        result: result_str.clone(),
        playback_start_time_ms: Utc::now().timestamp_millis(),
        steps_per_sec: STEPS_PER_SEC,
    };

    let _ = io.to(room.clone()).emit("match:start", &start_event);

    let result_event = MatchResultEvent {
        match_id: match_id.clone(),
        result: result_str.clone(),
        steps_taken,
        red_username: red.username.clone(),
        blue_username: blue.username.clone(),
    };

    let _ = io.to(room).emit("match:result", &result_event);

    info!(
        "match complete: {} vs {} -> {} ({} steps) — rating: red {}->{}, blue {}->{}",
        red.username,
        blue.username,
        result_str,
        steps_taken,
        outcome.red_rating_before,
        outcome.red_rating_after,
        outcome.blue_rating_before,
        outcome.blue_rating_after,
    );
}

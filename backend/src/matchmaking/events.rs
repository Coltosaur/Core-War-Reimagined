use crate::auth::socket::{get_auth, require_auth};
use crate::matchmaking::queue::{QueueEntry, RedisQueue};
use crate::models::warrior::Warrior;
use core_war_engine::{parse_warrior, MatchResult, MatchState};
use serde::{Deserialize, Serialize};
use socketioxide::extract::{Data, SocketRef};
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

const CORE_SIZE: usize = 8000;
const MAX_STEPS: u64 = 80_000;

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
struct MatchResultEvent {
    match_id: String,
    result: String,
    steps_taken: u64,
    red_username: String,
    blue_username: String,
}

pub fn register_events(socket: &SocketRef, queue: RedisQueue, db: PgPool) {
    let q = queue.clone();
    let pool = db.clone();
    socket.on(
        "queue:join",
        move |socket: SocketRef, Data(data): Data<QueueJoinData>| {
            let q = q.clone();
            let pool = pool.clone();
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
                        run_matched_battle(socket, pool, red, blue).await;
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

fn emit_to_socket(socket: &SocketRef, target_sid: &str, event: &str, data: &impl Serialize) {
    if socket.id.to_string() == target_sid {
        let _ = socket.emit(event, data);
    } else {
        let _ = socket.to(target_sid.to_string()).emit(event, data);
    }
}

async fn run_matched_battle(socket: SocketRef, db: PgPool, red: QueueEntry, blue: QueueEntry) {
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

    let found_event = MatchFound {
        match_id: match_id.clone(),
        red_username: red.username.clone(),
        blue_username: blue.username.clone(),
        red_warrior: red.warrior_id.to_string(),
        blue_warrior: blue.warrior_id.to_string(),
    };

    emit_to_socket(&socket, &red.socket_id, "match:found", &found_event);
    emit_to_socket(&socket, &blue.socket_id, "match:found", &found_event);

    let battle_result = tokio::task::spawn_blocking(move || {
        let red_parsed = parse_warrior(&red_source).map_err(|e| format!("Red parse: {e}"))?;
        let blue_parsed = parse_warrior(&blue_source).map_err(|e| format!("Blue parse: {e}"))?;

        let mut m = MatchState::new(CORE_SIZE, MAX_STEPS);
        m.load_warrior(0, &red_parsed, 0);
        m.load_warrior(1, &blue_parsed, CORE_SIZE / 2);

        while m.step() {}

        let result_str = match m.result() {
            MatchResult::Victory { winner_id: 0 } => "red_win",
            MatchResult::Victory { .. } => "blue_win",
            MatchResult::Tie => "tie",
            MatchResult::AllDead => "all_dead",
            MatchResult::Ongoing => unreachable!(),
        };

        Ok::<_, String>((result_str.to_string(), m.steps()))
    })
    .await;

    let (result_str, steps_taken) = match battle_result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            error!("Battle error: {e}");
            return;
        }
        Err(e) => {
            error!("Battle task panic: {e}");
            return;
        }
    };

    let result_event = MatchResultEvent {
        match_id,
        result: result_str.clone(),
        steps_taken,
        red_username: red.username.clone(),
        blue_username: blue.username.clone(),
    };

    emit_to_socket(&socket, &red.socket_id, "match:result", &result_event);
    emit_to_socket(&socket, &blue.socket_id, "match:result", &result_event);

    let _ = sqlx::query(
        "INSERT INTO matches \
           (red_warrior_id, blue_warrior_id, red_user_id, blue_user_id, \
            core_size, max_steps, result, steps_taken) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(red.warrior_id)
    .bind(blue.warrior_id)
    .bind(red.user_id)
    .bind(blue.user_id)
    .bind(CORE_SIZE as i32)
    .bind(MAX_STEPS as i32)
    .bind(&result_str)
    .bind(steps_taken as i32)
    .execute(&db)
    .await;

    info!(
        "match complete: {} vs {} -> {} ({} steps)",
        red.username, blue.username, result_str, steps_taken
    );
}

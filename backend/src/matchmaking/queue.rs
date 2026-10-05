use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub user_id: Uuid,
    pub username: String,
    pub warrior_id: Uuid,
    pub socket_id: String,
}

// Joining while already queued replaces the existing entry in place, keeping
// the player's position. The old entry may belong to a connection that has
// since dropped (network blip, server restart); rejecting the join would leave
// the player "queued" on a socket that can never be matched (#127).
const JOIN_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local users_key = KEYS[2]
local user_id = ARGV[1]
local entry_json = ARGV[2]

if redis.call('SISMEMBER', users_key, user_id) == 1 then
    local entries = redis.call('LRANGE', queue_key, 0, -1)
    for i, entry in ipairs(entries) do
        if cjson.decode(entry).user_id == user_id then
            redis.call('LSET', queue_key, i - 1, entry_json)
            return entry
        end
    end
    -- In the user set but not the list: fall through and queue normally.
end

redis.call('SADD', users_key, user_id)
redis.call('RPUSH', queue_key, entry_json)

if redis.call('LLEN', queue_key) >= 2 then
    local red = redis.call('LPOP', queue_key)
    local blue = redis.call('LPOP', queue_key)
    local red_data = cjson.decode(red)
    local blue_data = cjson.decode(blue)
    redis.call('SREM', users_key, red_data.user_id)
    redis.call('SREM', users_key, blue_data.user_id)
    return {red, blue}
end

return 1
"#;

// ARGV[2], when non-empty, only removes the entry if it belongs to that
// socket. A disconnecting socket must not remove an entry a newer connection
// of the same user has since replaced it with.
const LEAVE_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local users_key = KEYS[2]
local user_id = ARGV[1]
local socket_id = ARGV[2]

if redis.call('SISMEMBER', users_key, user_id) == 0 then
    return false
end

local entries = redis.call('LRANGE', queue_key, 0, -1)
for i, entry in ipairs(entries) do
    local data = cjson.decode(entry)
    if data.user_id == user_id then
        if socket_id ~= '' and data.socket_id ~= socket_id then
            return false
        end
        redis.call('LREM', queue_key, 1, entry)
        redis.call('SREM', users_key, user_id)
        return true
    end
end

redis.call('SREM', users_key, user_id)
return true
"#;

const POSITION_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local user_id = ARGV[1]

local entries = redis.call('LRANGE', queue_key, 0, -1)
for i, entry in ipairs(entries) do
    local data = cjson.decode(entry)
    if data.user_id == user_id then
        return i - 1
    end
end

return false
"#;

#[derive(Debug)]
pub enum JoinOutcome {
    /// Waiting for an opponent.
    Queued,
    /// The player was already queued; their entry now points at the new
    /// socket. `previous` is the entry it replaced.
    Replaced { previous: QueueEntry },
    /// Paired: (red, blue), both already removed from the queue.
    Matched(QueueEntry, QueueEntry),
}

#[derive(Clone)]
pub struct RedisQueue {
    conn: ConnectionManager,
    queue_key: String,
    users_key: String,
}

impl RedisQueue {
    pub fn new(conn: ConnectionManager) -> Self {
        Self {
            conn,
            queue_key: "matchmaking:queue".into(),
            users_key: "matchmaking:users".into(),
        }
    }

    #[cfg(test)]
    fn with_prefix(conn: ConnectionManager, prefix: &str) -> Self {
        Self {
            conn,
            queue_key: format!("{prefix}:queue"),
            users_key: format!("{prefix}:users"),
        }
    }

    pub async fn join(&self, entry: QueueEntry) -> Result<JoinOutcome, redis::RedisError> {
        let entry_json =
            serde_json::to_string(&entry).expect("QueueEntry serialization cannot fail");
        let mut conn = self.conn.clone();
        let result: redis::Value = redis::Script::new(JOIN_SCRIPT)
            .key(&self.queue_key)
            .key(&self.users_key)
            .arg(entry.user_id.to_string())
            .arg(&entry_json)
            .invoke_async(&mut conn)
            .await?;

        match result {
            redis::Value::Array(ref arr) if arr.len() == 2 => {
                let red = decode_entry(&arr[0])?;
                let blue = decode_entry(&arr[1])?;
                Ok(JoinOutcome::Matched(red, blue))
            }
            redis::Value::BulkString(_) => Ok(JoinOutcome::Replaced {
                previous: decode_entry(&result)?,
            }),
            _ => Ok(JoinOutcome::Queued),
        }
    }

    /// `join`, but never hands back a pair containing a player whose socket
    /// is gone. Entries outlive their sockets when the server restarts (the
    /// queue lives in Redis; `on_disconnect` never runs), and pairing a live
    /// player with one used to abort the match and silently drop both (#127).
    /// A dead entry is discarded and the live player is queued again.
    pub async fn join_live(
        &self,
        entry: QueueEntry,
        is_live: impl Fn(&QueueEntry) -> bool,
    ) -> Result<JoinOutcome, redis::RedisError> {
        let mut outcome = self.join(entry).await?;
        // Each pass discards at least one dead entry, so this terminates.
        loop {
            let JoinOutcome::Matched(red, blue) = outcome else {
                return Ok(outcome);
            };
            let survivor = match (is_live(&red), is_live(&blue)) {
                (true, true) => return Ok(JoinOutcome::Matched(red, blue)),
                (true, false) => Some(red),
                (false, true) => Some(blue),
                (false, false) => None,
            };
            let Some(survivor) = survivor else {
                return Ok(JoinOutcome::Queued);
            };
            outcome = self.join(survivor).await?;
        }
    }

    pub async fn leave(&self, user_id: Uuid) -> Result<bool, redis::RedisError> {
        self.run_leave(user_id, "").await
    }

    /// Remove the user's entry only if it belongs to `socket_id`.
    pub async fn leave_socket(
        &self,
        user_id: Uuid,
        socket_id: &str,
    ) -> Result<bool, redis::RedisError> {
        self.run_leave(user_id, socket_id).await
    }

    async fn run_leave(&self, user_id: Uuid, socket_id: &str) -> Result<bool, redis::RedisError> {
        let mut conn = self.conn.clone();
        let result: bool = redis::Script::new(LEAVE_SCRIPT)
            .key(&self.queue_key)
            .key(&self.users_key)
            .arg(user_id.to_string())
            .arg(socket_id)
            .invoke_async(&mut conn)
            .await?;
        Ok(result)
    }

    pub async fn position(&self, user_id: Uuid) -> Result<Option<usize>, redis::RedisError> {
        let mut conn = self.conn.clone();
        let result: redis::Value = redis::Script::new(POSITION_SCRIPT)
            .key(&self.queue_key)
            .arg(user_id.to_string())
            .invoke_async(&mut conn)
            .await?;

        match result {
            redis::Value::Int(pos) => Ok(Some(pos as usize)),
            _ => Ok(None),
        }
    }

    pub async fn len(&self) -> Result<usize, redis::RedisError> {
        let mut conn = self.conn.clone();
        conn.llen(&self.queue_key).await
    }

    pub async fn is_empty(&self) -> Result<bool, redis::RedisError> {
        Ok(self.len().await? == 0)
    }

    #[cfg(test)]
    async fn clear(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL")
            .arg(&self.queue_key)
            .arg(&self.users_key)
            .query_async(&mut conn)
            .await
    }
}

fn decode_entry(value: &redis::Value) -> Result<QueueEntry, redis::RedisError> {
    let json: String = redis::from_redis_value(value)?;
    serde_json::from_str(&json).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::IoError,
            "queue entry deserialization failed",
            e.to_string(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_queue(prefix: &str) -> RedisQueue {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
        let client = redis::Client::open(url.as_str()).unwrap();
        let conn = ConnectionManager::new(client).await.unwrap();
        let q = RedisQueue::with_prefix(conn, &format!("test:{prefix}"));
        q.clear().await.unwrap();
        q
    }

    fn entry(name: &str, sid: &str) -> QueueEntry {
        QueueEntry {
            user_id: Uuid::new_v4(),
            username: name.into(),
            warrior_id: Uuid::new_v4(),
            socket_id: sid.into(),
        }
    }

    #[tokio::test]
    async fn single_entry_no_match() {
        let q = test_queue("single").await;
        assert!(matches!(
            q.join(entry("alice", "s1")).await.unwrap(),
            JoinOutcome::Queued
        ));
        assert_eq!(q.len().await.unwrap(), 1);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn two_entries_match() {
        let q = test_queue("two_match").await;
        q.join(entry("alice", "s1")).await.unwrap();
        let JoinOutcome::Matched(red, blue) = q.join(entry("bob", "s2")).await.unwrap() else {
            panic!("expected a match");
        };
        assert_eq!(red.username, "alice");
        assert_eq!(blue.username, "bob");
        assert_eq!(q.len().await.unwrap(), 0);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn rejoin_replaces_entry_in_place() {
        let q = test_queue("rejoin").await;
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e).await.unwrap();
        let again = QueueEntry {
            user_id: uid,
            ..entry("alice", "s2")
        };
        let JoinOutcome::Replaced { previous } = q.join(again).await.unwrap() else {
            panic!("expected the entry to be replaced");
        };
        assert_eq!(previous.socket_id, "s1");
        assert_eq!(q.len().await.unwrap(), 1);
        assert_eq!(q.position(uid).await.unwrap(), Some(0));

        // The replacement, not the stale entry, is what gets matched.
        let JoinOutcome::Matched(red, _) = q.join(entry("bob", "s3")).await.unwrap() else {
            panic!("expected a match");
        };
        assert_eq!(red.socket_id, "s2");
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn stale_socket_disconnect_keeps_newer_entry() {
        let q = test_queue("leave_socket").await;
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e.clone()).await.unwrap();
        q.join(QueueEntry {
            socket_id: "s2".into(),
            ..e
        })
        .await
        .unwrap();

        assert!(!q.leave_socket(uid, "s1").await.unwrap());
        assert_eq!(q.position(uid).await.unwrap(), Some(0));
        assert!(q.leave_socket(uid, "s2").await.unwrap());
        assert_eq!(q.len().await.unwrap(), 0);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn dead_entry_is_discarded_and_live_player_requeued() {
        let q = test_queue("dead_entry").await;
        // Left behind by a server restart.
        q.join(entry("ghost", "dead")).await.unwrap();
        let is_live = |e: &QueueEntry| e.socket_id != "dead";

        let outcome = q.join_live(entry("alice", "s1"), is_live).await.unwrap();
        assert!(matches!(outcome, JoinOutcome::Queued), "{outcome:?}");
        assert_eq!(q.len().await.unwrap(), 1);

        let JoinOutcome::Matched(red, blue) =
            q.join_live(entry("bob", "s2"), is_live).await.unwrap()
        else {
            panic!("expected alice and bob to be matched");
        };
        assert_eq!(
            (red.username.as_str(), blue.username.as_str()),
            ("alice", "bob")
        );
        assert_eq!(q.len().await.unwrap(), 0);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn live_pair_is_matched() {
        let q = test_queue("live_pair").await;
        q.join(entry("alice", "s1")).await.unwrap();
        let outcome = q.join_live(entry("bob", "s2"), |_| true).await.unwrap();
        assert!(matches!(outcome, JoinOutcome::Matched(..)), "{outcome:?}");
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn leave_removes_entry() {
        let q = test_queue("leave").await;
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e).await.unwrap();
        assert!(q.leave(uid).await.unwrap());
        assert_eq!(q.len().await.unwrap(), 0);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn leave_nonexistent_returns_false() {
        let q = test_queue("leave_none").await;
        assert!(!q.leave(Uuid::new_v4()).await.unwrap());
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn position_tracking() {
        let q = test_queue("position").await;
        let e1 = entry("alice", "s1");
        let uid1 = e1.user_id;
        q.join(e1).await.unwrap();
        assert_eq!(q.position(uid1).await.unwrap(), Some(0));
        assert_eq!(q.position(Uuid::new_v4()).await.unwrap(), None);
        q.clear().await.unwrap();
    }
}

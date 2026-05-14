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

const JOIN_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local users_key = KEYS[2]
local user_id = ARGV[1]
local entry_json = ARGV[2]

if redis.call('SISMEMBER', users_key, user_id) == 1 then
    return false
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

const LEAVE_SCRIPT: &str = r#"
local queue_key = KEYS[1]
local users_key = KEYS[2]
local user_id = ARGV[1]

if redis.call('SREM', users_key, user_id) == 0 then
    return false
end

local entries = redis.call('LRANGE', queue_key, 0, -1)
for i, entry in ipairs(entries) do
    local data = cjson.decode(entry)
    if data.user_id == user_id then
        redis.call('LREM', queue_key, 1, entry)
        return true
    end
end

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

    pub async fn join(
        &self,
        entry: QueueEntry,
    ) -> Result<Option<(QueueEntry, QueueEntry)>, redis::RedisError> {
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
                let red_json: String = redis::from_redis_value(&arr[0])?;
                let blue_json: String = redis::from_redis_value(&arr[1])?;
                let red: QueueEntry = serde_json::from_str(&red_json).map_err(|e| {
                    redis::RedisError::from((
                        redis::ErrorKind::IoError,
                        "queue entry deserialization failed",
                        e.to_string(),
                    ))
                })?;
                let blue: QueueEntry = serde_json::from_str(&blue_json).map_err(|e| {
                    redis::RedisError::from((
                        redis::ErrorKind::IoError,
                        "queue entry deserialization failed",
                        e.to_string(),
                    ))
                })?;
                Ok(Some((red, blue)))
            }
            _ => Ok(None),
        }
    }

    pub async fn leave(&self, user_id: Uuid) -> Result<bool, redis::RedisError> {
        let mut conn = self.conn.clone();
        let result: bool = redis::Script::new(LEAVE_SCRIPT)
            .key(&self.queue_key)
            .key(&self.users_key)
            .arg(user_id.to_string())
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
        assert!(q.join(entry("alice", "s1")).await.unwrap().is_none());
        assert_eq!(q.len().await.unwrap(), 1);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn two_entries_match() {
        let q = test_queue("two_match").await;
        q.join(entry("alice", "s1")).await.unwrap();
        let result = q.join(entry("bob", "s2")).await.unwrap();
        assert!(result.is_some());
        let (red, blue) = result.unwrap();
        assert_eq!(red.username, "alice");
        assert_eq!(blue.username, "bob");
        assert_eq!(q.len().await.unwrap(), 0);
        q.clear().await.unwrap();
    }

    #[tokio::test]
    async fn duplicate_user_rejected() {
        let q = test_queue("dup").await;
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e).await.unwrap();
        let dup = QueueEntry {
            user_id: uid,
            username: "alice".into(),
            warrior_id: Uuid::new_v4(),
            socket_id: "s2".into(),
        };
        assert!(q.join(dup).await.unwrap().is_none());
        assert_eq!(q.len().await.unwrap(), 1);
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

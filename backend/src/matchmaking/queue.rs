use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub user_id: Uuid,
    pub username: String,
    pub warrior_id: Uuid,
    pub socket_id: String,
}

#[derive(Default)]
pub struct MatchmakingQueue {
    entries: Vec<QueueEntry>,
}

impl MatchmakingQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn join(&mut self, entry: QueueEntry) -> Option<(QueueEntry, QueueEntry)> {
        if self.entries.iter().any(|e| e.user_id == entry.user_id) {
            return None;
        }

        self.entries.push(entry);

        if self.entries.len() >= 2 {
            let blue = self.entries.remove(1);
            let red = self.entries.remove(0);
            Some((red, blue))
        } else {
            None
        }
    }

    pub fn leave(&mut self, user_id: Uuid) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.user_id != user_id);
        self.entries.len() < len
    }

    pub fn position(&self, user_id: Uuid) -> Option<usize> {
        self.entries.iter().position(|e| e.user_id == user_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub type SharedQueue = Arc<Mutex<MatchmakingQueue>>;

pub fn new_shared_queue() -> SharedQueue {
    Arc::new(Mutex::new(MatchmakingQueue::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, sid: &str) -> QueueEntry {
        QueueEntry {
            user_id: Uuid::new_v4(),
            username: name.into(),
            warrior_id: Uuid::new_v4(),
            socket_id: sid.into(),
        }
    }

    #[test]
    fn single_entry_no_match() {
        let mut q = MatchmakingQueue::new();
        assert!(q.join(entry("alice", "s1")).is_none());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn two_entries_match() {
        let mut q = MatchmakingQueue::new();
        q.join(entry("alice", "s1"));
        let result = q.join(entry("bob", "s2"));
        assert!(result.is_some());
        let (red, blue) = result.unwrap();
        assert_eq!(red.username, "alice");
        assert_eq!(blue.username, "bob");
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn duplicate_user_rejected() {
        let mut q = MatchmakingQueue::new();
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e);
        let dup = QueueEntry {
            user_id: uid,
            username: "alice".into(),
            warrior_id: Uuid::new_v4(),
            socket_id: "s2".into(),
        };
        assert!(q.join(dup).is_none());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn leave_removes_entry() {
        let mut q = MatchmakingQueue::new();
        let e = entry("alice", "s1");
        let uid = e.user_id;
        q.join(e);
        assert!(q.leave(uid));
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn leave_nonexistent_returns_false() {
        let mut q = MatchmakingQueue::new();
        assert!(!q.leave(Uuid::new_v4()));
    }

    #[test]
    fn position_tracking() {
        let mut q = MatchmakingQueue::new();
        let e1 = entry("alice", "s1");
        let uid1 = e1.user_id;
        q.join(e1);
        assert_eq!(q.position(uid1), Some(0));
        assert_eq!(q.position(Uuid::new_v4()), None);
    }
}

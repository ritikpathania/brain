#![allow(missing_docs)]

use crate::ha::consensus::models::*;

pub struct LeaderLeaseManager {
    current_term: u64,
    is_leader: bool,
    lease_expires_at: u64,
}

impl Default for LeaderLeaseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderLeaseManager {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            is_leader: false,
            lease_expires_at: 0,
        }
    }

    pub fn is_leader(&self, now: u64) -> bool {
        self.is_leader && now < self.lease_expires_at
    }

    pub fn current_term(&self) -> u64 {
        self.current_term
    }

    pub fn handle_event(&mut self, event: LeadershipEvent, now: u64, duration_secs: u64) {
        match event {
            LeadershipEvent::BecameLeader { term } => {
                self.current_term = term;
                self.is_leader = true;
                self.lease_expires_at = now + duration_secs;
            }
            LeadershipEvent::BecameFollower { term } => {
                self.current_term = term;
                self.is_leader = false;
                self.lease_expires_at = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leader_lease_manager_fencing_and_expiration() {
        let mut manager = LeaderLeaseManager::new();
        assert!(!manager.is_leader(1000));

        manager.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
        assert!(manager.is_leader(1002));
        assert!(!manager.is_leader(1006)); // Past 5s lease

        manager.handle_event(LeadershipEvent::BecameFollower { term: 2 }, 1002, 5);
        assert!(!manager.is_leader(1002)); // Immediately disabled on follower step-down
    }
}

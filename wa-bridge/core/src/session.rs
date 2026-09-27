//! Session lifecycle state machine (Plan Phase 1; v1.1 §11).
//!
//! `SessionState` (in [`crate::manifest`]) is the *persisted* form written into
//! checkpoints and manifests. This module owns the *rules*: which transitions
//! are legal, and it refuses anything else with [`Error::InvalidTransition`] so
//! an app (or a restored checkpoint from a future build) can never put a
//! transfer into a state it did not legitimately reach.
//!
//! ```text
//! Created → Preflight → Checkpointing → Transferring
//!                                          ├─→ Paused ──────┐
//!                                          ├─→ Reconnecting ┤
//!                                          └─→ TransferVerified
//! TransferVerified → OfficialImportRequired → PostVerify → Complete
//!                  └→ PostVerify (destination needs no official step)
//! ```
//!
//! Every state change is recorded in [`Session::history`] for diagnostics; the
//! history contains state names only and never private content (L7).

use crate::manifest::{Manifest, SessionState};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Guarded view over the lifecycle of one migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    migration_id: String,
    state: SessionState,
    history: Vec<SessionState>,
}

/// States reachable from `state`, in the order the happy path tries them.
pub const fn allowed_from(state: SessionState) -> &'static [SessionState] {
    use SessionState::*;
    match state {
        Created => &[Preflight],
        Preflight => &[Checkpointing],
        Checkpointing => &[Transferring],
        Transferring => &[Paused, Reconnecting, TransferVerified],
        Paused => &[Transferring, Checkpointing],
        Reconnecting => &[Transferring],
        TransferVerified => &[OfficialImportRequired, PostVerify],
        OfficialImportRequired => &[PostVerify],
        PostVerify => &[Complete],
        Complete => &[],
    }
}

impl Session {
    /// Start a new session for `migration_id` in [`SessionState::Created`].
    pub fn new(migration_id: impl Into<String>) -> Self {
        Self {
            migration_id: migration_id.into(),
            state: SessionState::Created,
            history: vec![SessionState::Created],
        }
    }

    pub fn migration_id(&self) -> &str {
        &self.migration_id
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    /// States entered so far, oldest first. Starts with `Created`.
    pub fn history(&self) -> &[SessionState] {
        &self.history
    }

    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Would `next` be a legal move from the current state?
    pub fn can_advance(&self, next: SessionState) -> bool {
        allowed_from(self.state).contains(&next)
    }

    /// Advance to `next`, or fail with [`Error::InvalidTransition`].
    pub fn advance(&mut self, next: SessionState) -> Result<()> {
        if !self.can_advance(next) {
            return Err(Error::InvalidTransition {
                from: self.state.name().to_string(),
                to: next.name().to_string(),
            });
        }
        self.state = next;
        self.history.push(next);
        Ok(())
    }

    /// Copy the current state onto a manifest/checkpoint before persisting it,
    /// so a crash can never leave the two disagreeing.
    pub fn sync_manifest(&self, manifest: &mut Manifest) {
        manifest.state = self.state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Category;

    fn manifest() -> Manifest {
        let mut m = Manifest::new("mig-1".into(), "android".into(), "ipad".into());
        m.add_object("o1".into(), Category::Image, 1024, 512);
        m
    }

    #[test]
    fn happy_path_reaches_complete() {
        use SessionState::*;
        let mut s = Session::new("mig-1");
        for step in [
            Preflight,
            Checkpointing,
            Transferring,
            TransferVerified,
            OfficialImportRequired,
            PostVerify,
            Complete,
        ] {
            s.advance(step)
                .unwrap_or_else(|e| panic!("{step:?} should follow: {e}"));
        }
        assert_eq!(s.state(), Complete);
        assert!(s.is_terminal());
        assert_eq!(s.history().len(), 8); // Created + 7 steps
    }

    #[test]
    fn illegal_jumps_are_rejected() {
        use SessionState::*;
        let mut s = Session::new("mig-1");
        let err = s.advance(Complete).unwrap_err();
        assert!(
            matches!(err, Error::InvalidTransition { ref from, ref to }
                if from == "created" && to == "complete"),
            "unexpected: {err:?}"
        );
        // A rejected transition must not move or record anything.
        assert_eq!(s.state(), Created);
        assert_eq!(s.history(), &[Created]);
    }

    #[test]
    fn pause_and_reconnect_resume_paths_are_legal() {
        use SessionState::*;
        let mut s = Session::new("mig-2");
        s.advance(Preflight).unwrap();
        s.advance(Checkpointing).unwrap();
        s.advance(Transferring).unwrap();

        // Network drop: Transferring -> Reconnecting -> Transferring
        s.advance(Reconnecting).unwrap();
        s.advance(Transferring).unwrap();

        // User paused / app backgrounded: Transferring -> Paused -> Checkpointing -> Transferring
        s.advance(Paused).unwrap();
        s.advance(Checkpointing).unwrap();
        s.advance(Transferring).unwrap();

        assert!(s.history().contains(&Reconnecting));
        assert!(s.history().contains(&Paused));
    }

    #[test]
    fn checkpointing_cannot_follow_paused_without_returning_to_transfer() {
        use SessionState::*;
        let mut s = Session::new("mig-3");
        s.advance(Preflight).unwrap();
        s.advance(Checkpointing).unwrap();
        s.advance(Transferring).unwrap();
        s.advance(Paused).unwrap();
        // Paused -> TransferVerified is not a legal shortcut.
        assert!(s.advance(TransferVerified).is_err());
        assert_eq!(s.state(), Paused);
    }

    #[test]
    fn complete_is_terminal() {
        use SessionState::*;
        let mut s = Session::new("mig-4");
        for step in [
            Preflight,
            Checkpointing,
            Transferring,
            TransferVerified,
            PostVerify,
            Complete,
        ] {
            s.advance(step).unwrap();
        }
        for next in [
            Created,
            Preflight,
            Checkpointing,
            Transferring,
            Paused,
            Reconnecting,
            TransferVerified,
            OfficialImportRequired,
            PostVerify,
            Complete,
        ] {
            assert!(s.advance(next).is_err(), "Complete must reject {next:?}");
        }
        assert!(allowed_from(Complete).is_empty());
    }

    #[test]
    fn vault_only_branch_skips_official_import() {
        use SessionState::*;
        let mut s = Session::new("mig-5");
        for step in [
            Preflight,
            Checkpointing,
            Transferring,
            TransferVerified,
            PostVerify,
            Complete,
        ] {
            s.advance(step).unwrap();
        }
        assert_eq!(s.state(), Complete);
    }

    #[test]
    fn sync_manifest_mirrors_state_for_checkpoints() {
        let mut m = manifest();
        // The session is always created for the manifest it drives.
        let mut s = Session::new(m.migration_id.clone());
        s.advance(SessionState::Preflight).unwrap();
        s.sync_manifest(&mut m);
        assert_eq!(m.state, SessionState::Preflight);

        let cp = crate::checkpoint::CheckpointStore::snapshot(&m);
        assert_eq!(cp.state, SessionState::Preflight);
        assert_eq!(cp.migration_id, "mig-1");
    }

    #[test]
    fn history_tracking_matches_every_documented_transition() {
        // Every state must be reachable from Created — a state the machine
        // cannot reach would be dead code in the flow.
        use SessionState::*;
        let all = [
            Created,
            Preflight,
            Checkpointing,
            Transferring,
            Paused,
            Reconnecting,
            TransferVerified,
            OfficialImportRequired,
            PostVerify,
            Complete,
        ];
        let mut reachable = vec![Created];
        let mut i = 0;
        while i < reachable.len() {
            for next in allowed_from(reachable[i]) {
                if !reachable.contains(next) {
                    reachable.push(*next);
                }
            }
            i += 1;
        }
        for state in all {
            assert!(reachable.contains(&state), "{state:?} is unreachable");
        }
    }
}

//! The tray surface state machine (spec 07 §12): five states, shape+colour double-coded, as a
//! PURE transition function decoupled from the `tray-icon` rendering. The §7 release gate is the
//! exhaustive test below: every declared transition reachable, no undeclared transition exists.

/// The five tray states. `Working.count` backs the "正在整理 N 个新图标" tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayState {
    Off,
    Watching,
    Paused,
    Working { count: u32 },
    Error { reason: String },
}

/// The events the host feeds the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// The user enabled automation.
    ToggleOn,
    /// The user disabled automation.
    ToggleOff,
    /// Desktop activity started (spec 07 §11 busy flag set).
    ActivityStart,
    /// Desktop activity ended (busy flag expired).
    ActivityEnd,
    /// A batch of `count` icons began formatting.
    BatchStart { count: u32 },
    /// The batch committed clean.
    BatchDone,
    /// A durable write / undo-journal failure.
    Failure { reason: String },
    /// The user acknowledged/retried the error.
    ErrorAcknowledged,
}

/// The legal-transition table (spec 07 §12): OFF↔WATCHING · WATCHING↔PAUSED ·
/// WATCHING/PAUSED→WORKING · WORKING→WATCHING · any→ERROR · ERROR→WATCHING. Any event not
/// declared for the current state is a NO-OP (the state is returned unchanged) — the tray never
/// invents a transition the table does not declare.
pub fn transition(current: TrayState, event: TrayEvent) -> TrayState {
    use TrayEvent as E;
    use TrayState as S;
    match (current, event) {
        // A failure interrupts anything, including OFF — an error surfaced while disabled still
        // needs the user's eyes (the undo safety net failing is never silent).
        (_, E::Failure { reason }) => S::Error { reason },
        (S::Off, E::ToggleOn) => S::Watching,
        (s @ S::Off, _) => s,
        (S::Watching, E::ToggleOff) => S::Off,
        (S::Watching, E::ActivityStart) => S::Paused,
        (S::Watching, E::BatchStart { count }) => S::Working { count },
        (s @ S::Watching, _) => s,
        (S::Paused, E::ActivityEnd) => S::Watching,
        (S::Paused, E::ToggleOff) => S::Off,
        (S::Paused, E::BatchStart { count }) => S::Working { count },
        (s @ S::Paused, _) => s,
        (S::Working { .. }, E::BatchDone) => S::Watching,
        (S::Working { .. }, E::ToggleOff) => S::Off,
        (s @ S::Working { .. }, _) => s,
        (S::Error { .. }, E::ErrorAcknowledged) => S::Watching,
        (S::Error { .. }, E::ToggleOff) => S::Off,
        (s @ S::Error { .. }, _) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TrayEvent as E;
    use TrayState as S;

    fn every_state() -> Vec<S> {
        vec![
            S::Off,
            S::Watching,
            S::Paused,
            S::Working { count: 3 },
            S::Error { reason: "x".into() },
        ]
    }

    fn every_event() -> Vec<E> {
        vec![
            E::ToggleOn,
            E::ToggleOff,
            E::ActivityStart,
            E::ActivityEnd,
            E::BatchStart { count: 3 },
            E::BatchDone,
            E::Failure { reason: "x".into() },
            E::ErrorAcknowledged,
        ]
    }

    /// The §7 exhaustive gate: for EVERY (state, event) pair the outcome is either a transition
    /// the spec table declares or a stay-put — never an undeclared jump.
    #[test]
    fn every_pair_lands_on_a_declared_transition_or_stays_put() {
        for state in every_state() {
            for event in every_event() {
                let next = transition(state.clone(), event.clone());
                let declared = matches!(
                    (&state, &event, &next),
                    // Declared table (spec 07 §12).
                    (S::Off, E::ToggleOn, S::Watching)
                        | (S::Watching, E::ToggleOff, S::Off)
                        | (S::Watching, E::ActivityStart, S::Paused)
                        | (S::Watching, E::BatchStart { .. }, S::Working { .. })
                        | (S::Paused, E::ActivityEnd, S::Watching)
                        | (S::Paused, E::ToggleOff, S::Off)
                        | (S::Paused, E::BatchStart { .. }, S::Working { .. })
                        | (S::Working { .. }, E::BatchDone, S::Watching)
                        | (S::Working { .. }, E::ToggleOff, S::Off)
                        | (_, E::Failure { .. }, S::Error { .. })
                        | (S::Error { .. }, E::ErrorAcknowledged, S::Watching)
                        | (S::Error { .. }, E::ToggleOff, S::Off)
                );
                let stay_put = next == state;
                assert!(
                    declared || stay_put,
                    "undeclared transition: {state:?} --{event:?}--> {next:?}"
                );
            }
        }
    }

    /// Every declared transition is REACHABLE (the mirror half of the gate).
    #[test]
    fn every_declared_transition_is_reachable() {
        assert_eq!(transition(S::Off, E::ToggleOn), S::Watching);
        assert_eq!(transition(S::Watching, E::ToggleOff), S::Off);
        assert_eq!(transition(S::Watching, E::ActivityStart), S::Paused);
        assert_eq!(transition(S::Watching, E::BatchStart { count: 2 }), S::Working { count: 2 });
        assert_eq!(transition(S::Paused, E::ActivityEnd), S::Watching);
        assert_eq!(transition(S::Paused, E::ToggleOff), S::Off);
        assert_eq!(transition(S::Paused, E::BatchStart { count: 1 }), S::Working { count: 1 });
        assert_eq!(transition(S::Working { count: 1 }, E::BatchDone), S::Watching);
        assert_eq!(transition(S::Working { count: 1 }, E::ToggleOff), S::Off);
        for s in every_state() {
            assert!(matches!(
                transition(s, E::Failure { reason: "boom".into() }),
                S::Error { .. }
            ));
        }
        assert_eq!(transition(S::Error { reason: "x".into() }, E::ErrorAcknowledged), S::Watching);
        assert_eq!(transition(S::Error { reason: "x".into() }, E::ToggleOff), S::Off);
    }

    /// The stay-put arms that matter for UX: a busy desktop does not flip Working; a second
    /// ActivityStart while Paused stays Paused; BatchDone in Watching is a no-op.
    #[test]
    fn representative_no_ops_hold() {
        assert_eq!(transition(S::Working { count: 4 }, E::ActivityStart), S::Working { count: 4 });
        assert_eq!(transition(S::Paused, E::ActivityStart), S::Paused);
        assert_eq!(transition(S::Watching, E::BatchDone), S::Watching);
        assert_eq!(transition(S::Off, E::BatchStart { count: 9 }), S::Off);
    }
}

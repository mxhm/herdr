use super::super::AgentState;

// omp is a fork of pi and shares both the working indicator and the pi-tui
// select dialog used by the `ask` tool and plan-mode approval, so detection is
// identical. Delegate to pi to keep the two in lockstep (Blocked is checked
// before Working there — a stale spinner row often lingers above a live
// select dialog).
pub(super) fn detect(content: &str) -> AgentState {
    super::pi::detect(content)
}

#[cfg(test)]
mod tests {
    use super::detect;
    use crate::detect::AgentState;

    // Verbatim capture from omp 15.5.10 on mondo (Qwen3.6-27B): the `ask` tool's
    // select dialog. Note the stale "Working… (esc to interrupt)" status row the
    // long-task extension leaves above the live dialog — Blocked must still win.
    const OMP_ASK_DIALOG: &str = "\
 ⠋ Working… (esc to interrupt)

   Todos
   └ I. Awaiting Selection

────────────────────────────────────────────────────────────────────────────

 DB choice?

────────────────────────────────────────────────────────────────────────────
│❯ PostgreSQL (Recommended)                                                  │
│  SQLite                                                                     │
│  Other (type your own)                                                      │
────────────────────────────────────────────────────────────────────────────

 up/down navigate  enter select  esc cancel

────────────────────────────────────────────────────────────────────────────";

    #[test]
    fn working_unicode_ellipsis_spinner() {
        assert_eq!(
            detect("output\n ⠋ Working… (esc to interrupt)"),
            AgentState::Working
        );
    }

    #[test]
    fn idle_no_working_text() {
        assert_eq!(detect("some output\n\n> ready"), AgentState::Idle);
    }

    #[test]
    fn blocked_ask_tool_select_dialog() {
        // The capture contains a lingering "Working…" row; Blocked must override.
        assert_eq!(detect(OMP_ASK_DIALOG), AgentState::Blocked);
    }

    #[test]
    fn blocked_multi_question_nav_chrome() {
        // Multi-question asks insert the ←/→ hint into the footer.
        let screen = " up/down navigate  enter select  ←/→ question  esc cancel";
        assert_eq!(detect(screen), AgentState::Blocked);
    }

    #[test]
    fn working_does_not_false_match_blocked() {
        // A plain working spinner without the dialog footer stays Working.
        assert_eq!(
            detect("output\n ⠋ Working… (esc to interrupt)"),
            AgentState::Working
        );
    }
}

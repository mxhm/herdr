use super::super::AgentState;

// pi/omp render a spinner line like "⠋ Working… (esc to interrupt)" while
// streaming. Match the stable parts: the trailing interrupt hint (shown only
// while the turn is interruptible) and the "Working" label. Tolerate BOTH the
// Unicode ellipsis "…" (U+2026, what current pi/omp emit) and the older ASCII
// "..." — a bare "Working..." literal match silently broke when the TUI
// switched to the ellipsis glyph, leaving the pane stuck on Idle.
pub(super) fn working(content: &str) -> bool {
    content.contains("esc to interrupt")
        || content.contains("Working…")
        || content.contains("Working...")
}

// pi/omp render agent-initiated, wait-on-user prompts (the `ask` tool and the
// plan-mode approval menu) through the shared pi-tui select dialog. Every such
// dialog draws the same footer help line; the "navigate" + "enter select" pair
// is always present and is absent from the Working spinner and the Idle prompt
// box. This MUST win over `working`: when the dialog is up a stale
// "Working… (esc to interrupt)" row often lingers in the scrollback above it.
pub(super) fn blocked(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("enter select") && lower.contains("navigate")
}

pub(super) fn detect(content: &str) -> AgentState {
    if blocked(content) {
        return AgentState::Blocked;
    }
    if working(content) {
        return AgentState::Working;
    }
    AgentState::Idle
}

#[cfg(test)]
mod tests {
    use super::detect;
    use crate::detect::AgentState;

    #[test]
    fn working_ascii() {
        assert_eq!(detect("some output\nWorking..."), AgentState::Working);
    }

    #[test]
    fn working_unicode_ellipsis() {
        assert_eq!(detect("⠋ Working… (esc to interrupt)"), AgentState::Working);
    }

    #[test]
    fn idle_at_prompt() {
        assert_eq!(detect("❯ "), AgentState::Idle);
    }

    #[test]
    fn blocked_select_dialog() {
        assert_eq!(
            detect(" up/down navigate  enter select  esc cancel"),
            AgentState::Blocked
        );
    }
}

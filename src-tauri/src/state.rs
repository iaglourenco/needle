use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SessionState {
    Running,
    WaitingInput,
    NeedsAttention,
    Idle,
    Error,
    Stale,
    Ended,
}

impl SessionState {
    /// Ordena estados por severidade pra escolher o pior estado agregado
    /// entre todas as sessões ativas (usado pelo ícone da bandeja).
    pub fn severity(self) -> u8 {
        match self {
            SessionState::Error => 5,
            SessionState::NeedsAttention => 4,
            SessionState::WaitingInput => 3,
            SessionState::Running => 2,
            SessionState::Idle => 1,
            SessionState::Stale => 0,
            SessionState::Ended => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse { is_error: bool },
    Notification,
    Stop,
    SubagentStop,
    SessionEnd,
    PreCompact,
}

/// Função pura: dado o estado atual e um evento recebido, retorna o novo
/// estado. Não depende de tempo/relógio (isso é responsabilidade do job de
/// limpeza, ver `apply_waiting_timeout` / `apply_stale_timeout`).
pub fn transition(_current: SessionState, event: HookEvent) -> SessionState {
    match event {
        HookEvent::SessionStart => SessionState::Running,
        HookEvent::UserPromptSubmit => SessionState::Running,
        HookEvent::PreToolUse => SessionState::Running,
        HookEvent::PostToolUse { is_error: true } => SessionState::Error,
        HookEvent::PostToolUse { is_error: false } => SessionState::Running,
        HookEvent::Notification => SessionState::WaitingInput,
        HookEvent::Stop => SessionState::Idle,
        HookEvent::SubagentStop => SessionState::Running,
        HookEvent::SessionEnd => SessionState::Ended,
        HookEvent::PreCompact => SessionState::Running,
    }
}

/// Chamado periodicamente pelo job de limpeza: promove WaitingInput pra
/// NeedsAttention depois de `threshold_secs` sem novo evento.
pub fn apply_waiting_timeout(
    current: SessionState,
    seconds_since_last_event: i64,
    threshold_secs: i64,
) -> SessionState {
    if current == SessionState::WaitingInput && seconds_since_last_event > threshold_secs {
        SessionState::NeedsAttention
    } else {
        current
    }
}

/// Chamado periodicamente pelo job de limpeza: sessão sem qualquer evento
/// há `stale_secs` e que não está esperando input vira Stale.
pub fn apply_stale_timeout(
    current: SessionState,
    seconds_since_last_event: i64,
    stale_secs: i64,
) -> SessionState {
    let is_waiting = matches!(
        current,
        SessionState::WaitingInput | SessionState::NeedsAttention
    );
    if !is_waiting && current != SessionState::Ended && seconds_since_last_event > stale_secs {
        SessionState::Stale
    } else {
        current
    }
}

pub fn worst_state(states: impl IntoIterator<Item = SessionState>) -> Option<SessionState> {
    states.into_iter().max_by_key(|s| s.severity())
}

/// Só sessões em estado terminal podem ser apagadas manualmente — evita
/// que uma sessão ativa suma por engano.
pub fn is_manually_deletable(current: Option<SessionState>) -> bool {
    matches!(current, Some(SessionState::Stale) | Some(SessionState::Ended))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_start_and_prompt_are_running() {
        assert_eq!(
            transition(SessionState::Idle, HookEvent::SessionStart),
            SessionState::Running
        );
        assert_eq!(
            transition(SessionState::Running, HookEvent::UserPromptSubmit),
            SessionState::Running
        );
    }

    #[test]
    fn notification_waits_for_input() {
        assert_eq!(
            transition(SessionState::Running, HookEvent::Notification),
            SessionState::WaitingInput
        );
    }

    #[test]
    fn stop_goes_idle() {
        assert_eq!(
            transition(SessionState::Running, HookEvent::Stop),
            SessionState::Idle
        );
    }

    #[test]
    fn post_tool_use_error_sets_error_state() {
        assert_eq!(
            transition(
                SessionState::Running,
                HookEvent::PostToolUse { is_error: true }
            ),
            SessionState::Error
        );
        assert_eq!(
            transition(
                SessionState::Running,
                HookEvent::PostToolUse { is_error: false }
            ),
            SessionState::Running
        );
    }

    #[test]
    fn session_end_ends_session() {
        assert_eq!(
            transition(SessionState::Idle, HookEvent::SessionEnd),
            SessionState::Ended
        );
    }

    #[test]
    fn waiting_input_promotes_to_needs_attention_after_threshold() {
        assert_eq!(
            apply_waiting_timeout(SessionState::WaitingInput, 30, 60),
            SessionState::WaitingInput
        );
        assert_eq!(
            apply_waiting_timeout(SessionState::WaitingInput, 61, 60),
            SessionState::NeedsAttention
        );
        assert_eq!(
            apply_waiting_timeout(SessionState::Running, 999, 60),
            SessionState::Running
        );
    }

    #[test]
    fn idle_session_goes_stale_after_timeout() {
        assert_eq!(
            apply_stale_timeout(SessionState::Idle, 1799, 1800),
            SessionState::Idle
        );
        assert_eq!(
            apply_stale_timeout(SessionState::Idle, 1801, 1800),
            SessionState::Stale
        );
    }

    #[test]
    fn waiting_input_never_goes_stale() {
        assert_eq!(
            apply_stale_timeout(SessionState::WaitingInput, 999_999, 1800),
            SessionState::WaitingInput
        );
        assert_eq!(
            apply_stale_timeout(SessionState::NeedsAttention, 999_999, 1800),
            SessionState::NeedsAttention
        );
    }

    #[test]
    fn ended_session_never_goes_stale() {
        assert_eq!(
            apply_stale_timeout(SessionState::Ended, 999_999, 1800),
            SessionState::Ended
        );
    }

    #[test]
    fn only_stale_and_ended_are_manually_deletable() {
        assert!(is_manually_deletable(Some(SessionState::Stale)));
        assert!(is_manually_deletable(Some(SessionState::Ended)));
        assert!(!is_manually_deletable(Some(SessionState::Running)));
        assert!(!is_manually_deletable(Some(SessionState::WaitingInput)));
        assert!(!is_manually_deletable(Some(SessionState::NeedsAttention)));
        assert!(!is_manually_deletable(Some(SessionState::Idle)));
        assert!(!is_manually_deletable(Some(SessionState::Error)));
        assert!(!is_manually_deletable(None));
    }

    #[test]
    fn worst_state_picks_highest_severity() {
        let states = vec![
            SessionState::Idle,
            SessionState::Running,
            SessionState::NeedsAttention,
            SessionState::WaitingInput,
        ];
        assert_eq!(worst_state(states), Some(SessionState::NeedsAttention));
        assert_eq!(worst_state(Vec::<SessionState>::new()), None);
    }
}

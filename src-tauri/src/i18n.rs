use crate::settings::Language;
use crate::state::SessionState;

pub fn menu_label(lang: Language, id: &str) -> &'static str {
    match (lang, id) {
        (Language::PtBr, "open") => "Abrir Needle",
        (Language::En, "open") => "Open Needle",
        (Language::PtBr, "settings") => "Configurações",
        (Language::En, "settings") => "Settings",
        (Language::PtBr, "reconfigure") => "Reconfigurar hooks",
        (Language::En, "reconfigure") => "Reconfigure hooks",
        (Language::PtBr, "quit") => "Sair",
        (Language::En, "quit") => "Quit",
        _ => "",
    }
}

pub fn tray_tooltip(lang: Language, state: Option<SessionState>) -> String {
    match (lang, state) {
        (Language::PtBr, None) => "Needle — nenhuma sessão ativa".to_string(),
        (Language::En, None) => "Needle — no active sessions".to_string(),
        (Language::PtBr, Some(SessionState::NeedsAttention)) => {
            "Needle — sessão precisa de atenção".to_string()
        }
        (Language::En, Some(SessionState::NeedsAttention)) => {
            "Needle — session needs attention".to_string()
        }
        (Language::PtBr, Some(SessionState::Error)) => "Needle — erro em uma sessão".to_string(),
        (Language::En, Some(SessionState::Error)) => "Needle — error in a session".to_string(),
        (Language::PtBr, Some(SessionState::WaitingInput)) => {
            "Needle — aguardando input".to_string()
        }
        (Language::En, Some(SessionState::WaitingInput)) => {
            "Needle — waiting for input".to_string()
        }
        (Language::PtBr, Some(SessionState::Running)) => {
            "Needle — sessões em execução".to_string()
        }
        (Language::En, Some(SessionState::Running)) => "Needle — sessions running".to_string(),
        (Language::PtBr, Some(SessionState::Idle)) => "Needle — sessões ociosas".to_string(),
        (Language::En, Some(SessionState::Idle)) => "Needle — idle sessions".to_string(),
        (_, Some(SessionState::Stale | SessionState::Ended)) => "Needle".to_string(),
    }
}

pub fn notif_title(lang: Language, state: SessionState) -> &'static str {
    match (lang, state) {
        (Language::PtBr, SessionState::NeedsAttention) => "Needle: sessão precisa de atenção",
        (Language::En, SessionState::NeedsAttention) => "Needle: session needs attention",
        (Language::PtBr, SessionState::Error) => "Needle: erro numa sessão",
        (Language::En, SessionState::Error) => "Needle: error in a session",
        (Language::PtBr, SessionState::WaitingInput) => "Needle: aguardando sua resposta",
        (Language::En, SessionState::WaitingInput) => "Needle: waiting for your reply",
        _ => "Needle",
    }
}

pub fn hooks_configured_notification(lang: Language) -> (&'static str, &'static str) {
    match lang {
        Language::PtBr => (
            "Needle configurado",
            "Hooks do Claude Code registrados automaticamente.",
        ),
        Language::En => (
            "Needle configured",
            "Claude Code hooks registered automatically.",
        ),
    }
}

pub fn hooks_reconfigured_body(lang: Language) -> &'static str {
    match lang {
        Language::PtBr => "Hooks reconfigurados.",
        Language::En => "Hooks reconfigured.",
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_label, tray_tooltip, notif_title, hooks_configured_notification, hooks_reconfigured_body};
    use crate::settings::Language;
    use crate::state::SessionState;

    #[test]
    fn menu_labels_differ_by_language_and_are_non_empty() {
        for id in ["open", "settings", "reconfigure", "quit"] {
            let pt = menu_label(Language::PtBr, id);
            let en = menu_label(Language::En, id);
            assert!(!pt.is_empty(), "empty pt-BR label for {id}");
            assert!(!en.is_empty(), "empty en label for {id}");
            assert_ne!(pt, en, "identical label for {id}");
        }
    }

    #[test]
    fn tray_tooltip_differs_by_language() {
        let pt = tray_tooltip(Language::PtBr, None);
        let en = tray_tooltip(Language::En, None);
        assert!(!pt.is_empty());
        assert!(!en.is_empty());
        assert_ne!(pt, en);
    }

    #[test]
    fn tray_tooltip_is_needle_for_stale_and_ended_regardless_of_language() {
        assert_eq!(tray_tooltip(Language::PtBr, Some(SessionState::Stale)), "Needle");
        assert_eq!(tray_tooltip(Language::En, Some(SessionState::Ended)), "Needle");
    }

    #[test]
    fn notif_title_differs_by_language() {
        let pt = notif_title(Language::PtBr, SessionState::NeedsAttention);
        let en = notif_title(Language::En, SessionState::NeedsAttention);
        assert_ne!(pt, en);
    }

    #[test]
    fn hooks_configured_notification_differs_by_language() {
        let (pt_title, pt_body) = hooks_configured_notification(Language::PtBr);
        let (en_title, en_body) = hooks_configured_notification(Language::En);
        assert_ne!(pt_title, en_title);
        assert_ne!(pt_body, en_body);
    }

    #[test]
    fn hooks_reconfigured_body_differs_by_language() {
        assert_ne!(
            hooks_reconfigured_body(Language::PtBr),
            hooks_reconfigured_body(Language::En)
        );
    }
}

use crossterm::event::KeyCode;

use crate::config::{ActionKeybinds, CustomCommandKeybind, Keybinds};

use super::TerminalKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeybindDispatch {
    Direct,
    Prefix,
}

#[derive(Debug, Clone)]
pub(crate) enum KeybindMatch {
    Action(KeybindAction),
    Command(CustomCommandKeybind),
}

/// A resolved binding plus whether it opted in to tmux `bind -r` style repeat.
/// Only non-indexed action bindings dispatched from prefix mode can be
/// repeatable; custom commands and indexed bindings never are.
#[derive(Debug, Clone)]
pub(crate) struct PrefixKeybindMatch {
    pub binding: KeybindMatch,
    pub repeatable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeybindAction {
    NewWorkspace,
    NewWorktree,
    OpenWorktree,
    RemoveWorktree,
    RenameWorkspace,
    CloseWorkspace,
    SwitchWorkspace(usize),
    SwitchTab(usize),
    FocusAgent(usize),
    WorkspacePicker,
    PreviousWorkspace,
    NextWorkspace,
    PreviousAgent,
    NextAgent,
    NewTab,
    RenameTab,
    PreviousTab,
    NextTab,
    MoveTabPrevious,
    MoveTabNext,
    CloseTab,
    RenamePane,
    FocusPaneLeft,
    FocusPaneDown,
    FocusPaneUp,
    FocusPaneRight,
    SwapPaneLeft,
    SwapPaneDown,
    SwapPaneUp,
    SwapPaneRight,
    SplitVertical,
    SplitHorizontal,
    ClosePane,
    EditScrollback,
    CopyMode,
    Zoom,
    EnterResizeMode,
    ResizePaneLeft,
    ResizePaneDown,
    ResizePaneUp,
    ResizePaneRight,
    ToggleSidebar,
    CyclePaneNext,
    CyclePanePrevious,
    LastPane,
    Help,
    Settings,
    ReloadConfig,
    OpenNotificationTarget,
    Detach,
    OpenNavigator,
    ToggleAgentMark,
}

pub(crate) fn resolve_direct_binding(
    keybinds: &Keybinds,
    key: &TerminalKey,
) -> Option<KeybindMatch> {
    resolve_exact_binding(keybinds, key, KeybindDispatch::Direct)
}

/// Resolve a prefix-dispatched binding, reporting whether it opted in to
/// repeat. The generated-character fallback resolves repeat from the key that
/// actually matched.
pub(crate) fn resolve_prefix_binding_with_repeat(
    keybinds: &Keybinds,
    key: &TerminalKey,
) -> Option<PrefixKeybindMatch> {
    resolve_exact_binding_with_repeat(keybinds, key, KeybindDispatch::Prefix).or_else(|| {
        generated_character_key(key).and_then(|generated_key| {
            resolve_exact_binding_with_repeat(keybinds, &generated_key, KeybindDispatch::Prefix)
        })
    })
}

pub(crate) fn resolve_non_indexed_action(
    keybinds: &Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<KeybindAction> {
    resolve_non_indexed_binding(keybinds, key, dispatch).map(|(_bindings, action)| action)
}

/// Like [`resolve_non_indexed_action`], but also returns the matched
/// [`ActionKeybinds`] so callers can read per-binding options such as repeat.
///
/// The table below is scanned in order and the first action whose bindings
/// match wins. Two actions configured on the same key are not reported as a
/// conflict: the later one simply never fires, which reads to the user as that
/// action's key doing nothing. Keep that in mind when picking a default for a
/// newly added action.
fn resolve_non_indexed_binding<'a>(
    keybinds: &'a Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<(&'a ActionKeybinds, KeybindAction)> {
    for (bindings, action) in [
        (&keybinds.help, KeybindAction::Help),
        (&keybinds.settings, KeybindAction::Settings),
        (&keybinds.workspace_picker, KeybindAction::WorkspacePicker),
        (&keybinds.new_workspace, KeybindAction::NewWorkspace),
        (&keybinds.new_worktree, KeybindAction::NewWorktree),
        (&keybinds.open_worktree, KeybindAction::OpenWorktree),
        (&keybinds.remove_worktree, KeybindAction::RemoveWorktree),
        (&keybinds.rename_workspace, KeybindAction::RenameWorkspace),
        (&keybinds.close_workspace, KeybindAction::CloseWorkspace),
        (
            &keybinds.previous_workspace,
            KeybindAction::PreviousWorkspace,
        ),
        (&keybinds.next_workspace, KeybindAction::NextWorkspace),
        (&keybinds.previous_agent, KeybindAction::PreviousAgent),
        (&keybinds.next_agent, KeybindAction::NextAgent),
        (&keybinds.new_tab, KeybindAction::NewTab),
        (&keybinds.rename_tab, KeybindAction::RenameTab),
        (&keybinds.previous_tab, KeybindAction::PreviousTab),
        (&keybinds.next_tab, KeybindAction::NextTab),
        (&keybinds.move_tab_previous, KeybindAction::MoveTabPrevious),
        (&keybinds.move_tab_next, KeybindAction::MoveTabNext),
        (&keybinds.close_tab, KeybindAction::CloseTab),
        (&keybinds.rename_pane, KeybindAction::RenamePane),
        (&keybinds.edit_scrollback, KeybindAction::EditScrollback),
        (&keybinds.copy_mode, KeybindAction::CopyMode),
        (&keybinds.focus_pane_left, KeybindAction::FocusPaneLeft),
        (&keybinds.focus_pane_down, KeybindAction::FocusPaneDown),
        (&keybinds.focus_pane_up, KeybindAction::FocusPaneUp),
        (&keybinds.focus_pane_right, KeybindAction::FocusPaneRight),
        (&keybinds.swap_pane_left, KeybindAction::SwapPaneLeft),
        (&keybinds.swap_pane_down, KeybindAction::SwapPaneDown),
        (&keybinds.swap_pane_up, KeybindAction::SwapPaneUp),
        (&keybinds.swap_pane_right, KeybindAction::SwapPaneRight),
        (&keybinds.last_pane, KeybindAction::LastPane),
        (&keybinds.cycle_pane_next, KeybindAction::CyclePaneNext),
        (
            &keybinds.cycle_pane_previous,
            KeybindAction::CyclePanePrevious,
        ),
        (&keybinds.split_vertical, KeybindAction::SplitVertical),
        (&keybinds.split_horizontal, KeybindAction::SplitHorizontal),
        (&keybinds.close_pane, KeybindAction::ClosePane),
        (&keybinds.zoom, KeybindAction::Zoom),
        (&keybinds.resize_mode, KeybindAction::EnterResizeMode),
        (&keybinds.resize_pane_left, KeybindAction::ResizePaneLeft),
        (&keybinds.resize_pane_down, KeybindAction::ResizePaneDown),
        (&keybinds.resize_pane_up, KeybindAction::ResizePaneUp),
        (&keybinds.resize_pane_right, KeybindAction::ResizePaneRight),
        (&keybinds.toggle_sidebar, KeybindAction::ToggleSidebar),
        (&keybinds.reload_config, KeybindAction::ReloadConfig),
        (
            &keybinds.open_notification_target,
            KeybindAction::OpenNotificationTarget,
        ),
        (&keybinds.detach, KeybindAction::Detach),
        (&keybinds.goto, KeybindAction::OpenNavigator),
        (&keybinds.toggle_mark, KeybindAction::ToggleAgentMark),
    ] {
        if action_matches(bindings, key, dispatch) {
            return Some((bindings, action));
        }
    }
    None
}

pub(crate) fn resolve_custom_command(
    keybinds: &Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<CustomCommandKeybind> {
    keybinds
        .custom_commands
        .iter()
        .find(|binding| match dispatch {
            KeybindDispatch::Direct => binding.bindings.matches_direct_key(key),
            KeybindDispatch::Prefix => binding.bindings.matches_prefix_key(key),
        })
        .cloned()
}

pub(crate) fn resolve_indexed_action(
    keybinds: &Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<KeybindAction> {
    let actual_modifiers = crate::config::normalize_key_combo((key.code, key.modifiers)).1;

    for exact_modifiers in [true, false] {
        let trigger_matches = |binding: &crate::config::IndexedKeybind| {
            let dispatch_matches = match dispatch {
                KeybindDispatch::Direct => binding.trigger.is_direct(),
                KeybindDispatch::Prefix => binding.trigger.is_prefix(),
            };
            let expected_modifiers = crate::config::normalize_key_combo(binding.trigger.combo()).1;
            dispatch_matches && (actual_modifiers == expected_modifiers) == exact_modifiers
        };

        for binding in &keybinds.switch_tab {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::SwitchTab(index));
                }
            }
        }
        for binding in &keybinds.switch_workspace {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::SwitchWorkspace(index));
                }
            }
        }
        for binding in &keybinds.focus_agent {
            if trigger_matches(binding) {
                if let Some(index) = binding.matched_index(key) {
                    return Some(KeybindAction::FocusAgent(index));
                }
            }
        }
    }

    None
}

fn resolve_exact_binding(
    keybinds: &Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<KeybindMatch> {
    resolve_exact_binding_with_repeat(keybinds, key, dispatch).map(|resolved| resolved.binding)
}

fn resolve_exact_binding_with_repeat(
    keybinds: &Keybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> Option<PrefixKeybindMatch> {
    if let Some((bindings, action)) = resolve_non_indexed_binding(keybinds, key, dispatch) {
        return Some(PrefixKeybindMatch {
            binding: KeybindMatch::Action(action),
            repeatable: dispatch == KeybindDispatch::Prefix && bindings.repeatable_for_key(key),
        });
    }
    resolve_custom_command(keybinds, key, dispatch)
        .map(KeybindMatch::Command)
        .or_else(|| resolve_indexed_action(keybinds, key, dispatch).map(KeybindMatch::Action))
        .map(|binding| PrefixKeybindMatch {
            binding,
            // Custom commands and indexed bindings are never repeatable.
            repeatable: false,
        })
}

fn generated_character_key(key: &TerminalKey) -> Option<TerminalKey> {
    let mut characters = key.generated_text.as_deref()?.chars();
    let character = characters.next()?;
    if character.is_control() || characters.next().is_some() {
        return None;
    }
    Some(TerminalKey::new(
        KeyCode::Char(character),
        crossterm::event::KeyModifiers::empty(),
    ))
}

fn action_matches(
    bindings: &crate::config::ActionKeybinds,
    key: &TerminalKey,
    dispatch: KeybindDispatch,
) -> bool {
    match dispatch {
        KeybindDispatch::Direct => bindings.matches_direct_key(key),
        KeybindDispatch::Prefix => bindings.matches_prefix_key(key),
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyModifiers};

    use super::*;

    #[test]
    fn one_shared_resolver_handles_direct_prefix_and_indexed_bindings() {
        let keybinds = Keybinds {
            next_tab: crate::config::ActionKeybinds::direct("ctrl+n"),
            ..Keybinds::default()
        };

        let direct = TerminalKey::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        assert!(matches!(
            resolve_direct_binding(&keybinds, &direct),
            Some(KeybindMatch::Action(KeybindAction::NextTab))
        ));

        let help = TerminalKey::new(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(matches!(
            resolve_prefix_binding_with_repeat(&keybinds, &help).map(|resolved| resolved.binding),
            Some(KeybindMatch::Action(KeybindAction::Help))
        ));

        let one = TerminalKey::new(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(matches!(
            resolve_prefix_binding_with_repeat(&keybinds, &one).map(|resolved| resolved.binding),
            Some(KeybindMatch::Action(KeybindAction::SwitchTab(0)))
        ));
    }

    #[test]
    fn prefix_resolution_uses_shared_generated_character_fallback() {
        let keybinds = Keybinds::default();
        let key = TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
            .with_generated_text(Some("?".to_owned()));

        assert!(matches!(
            resolve_prefix_binding_with_repeat(&keybinds, &key).map(|resolved| resolved.binding),
            Some(KeybindMatch::Action(KeybindAction::Help))
        ));
    }

    fn keybinds_from_toml(toml: &str) -> Keybinds {
        toml::from_str::<crate::config::Config>(toml)
            .expect("config parses")
            .keybinds()
    }

    #[test]
    fn prefix_resolution_reports_table_form_repeat_opt_in() {
        let keybinds = keybinds_from_toml(
            r#"
[keys]
next_tab = { key = "prefix+n", repeat = true }
previous_tab = "prefix+p"
"#,
        );

        let next = TerminalKey::new(KeyCode::Char('n'), KeyModifiers::empty());
        let resolved = resolve_prefix_binding_with_repeat(&keybinds, &next).expect("next_tab");
        assert!(matches!(
            resolved.binding,
            KeybindMatch::Action(KeybindAction::NextTab)
        ));
        assert!(resolved.repeatable);

        let previous = TerminalKey::new(KeyCode::Char('p'), KeyModifiers::empty());
        let resolved =
            resolve_prefix_binding_with_repeat(&keybinds, &previous).expect("previous_tab");
        assert!(!resolved.repeatable);
    }

    #[test]
    fn direct_dispatch_never_reports_repeat() {
        let keybinds = keybinds_from_toml(
            r#"
[keys]
next_tab = { key = "ctrl+n", repeat = true }
"#,
        );

        let key = TerminalKey::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        let resolved = resolve_exact_binding_with_repeat(&keybinds, &key, KeybindDispatch::Direct)
            .expect("next_tab");
        assert!(!resolved.repeatable);
    }

    #[test]
    fn generated_character_fallback_keeps_repeat_opt_in() {
        let keybinds = keybinds_from_toml(
            r#"
[keys]
help = { key = "prefix+?", repeat = true }
"#,
        );
        let key = TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
            .with_generated_text(Some("?".to_owned()));

        let resolved = resolve_prefix_binding_with_repeat(&keybinds, &key).expect("help");
        assert!(matches!(
            resolved.binding,
            KeybindMatch::Action(KeybindAction::Help)
        ));
        assert!(resolved.repeatable);
    }

    #[test]
    fn custom_commands_and_indexed_bindings_are_never_repeatable() {
        let keybinds = keybinds_from_toml(
            r#"
[[keys.command]]
key = "prefix+y"
command = "lazygit"
"#,
        );

        let indexed = TerminalKey::new(KeyCode::Char('1'), KeyModifiers::empty());
        let resolved = resolve_prefix_binding_with_repeat(&keybinds, &indexed).expect("switch_tab");
        assert!(matches!(
            resolved.binding,
            KeybindMatch::Action(KeybindAction::SwitchTab(0))
        ));
        assert!(!resolved.repeatable);

        let command = TerminalKey::new(KeyCode::Char('y'), KeyModifiers::empty());
        let resolved = resolve_prefix_binding_with_repeat(&keybinds, &command).expect("command");
        assert!(matches!(resolved.binding, KeybindMatch::Command(_)));
        assert!(!resolved.repeatable);
    }
}

use super::*;

/// `repeat = true` bindings plus a non-repeatable one, a repeatable binding
/// that enters a sub-mode, and a repeatable binding that opens an overlay.
const REPEAT_KEYS_TOML: &str = r#"
[keys]
repeat_timeout = 750
next_tab = { key = "prefix+n", repeat = true }
previous_tab = "prefix+p"
resize_mode = { key = "prefix+r", repeat = true }
help = { key = "prefix+?", repeat = true }
"#;

fn two_tab_snapshot() -> ClientShellSnapshot {
    let mut snapshot = snapshot();
    snapshot.tabs.push(ClientShellTab {
        tab_id: "tab_2".into(),
        workspace_id: "ws_1".into(),
        number: 2,
        label: "2".into(),
        custom_label: false,
        zoomed: false,
        focused: false,
        agent_status: AgentStatus::Idle,
    });
    snapshot
}

fn state_with_keys(toml: &str) -> ClientShellState {
    let config: Config = toml::from_str(toml).expect("keybind config parses");
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(surface());
    state
}

fn repeat_state() -> ClientShellState {
    state_with_keys(REPEAT_KEYS_TOML)
}

fn tab_focus_targets(outcome: &ClientShellInput) -> Vec<String> {
    outcome
        .actions
        .iter()
        .filter_map(|action| match action {
            ClientShellAction::Endpoint { request, .. } => match &request.method {
                crate::api::schema::Method::TabFocus(target) => Some(target.tab_id.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn repeatable_prefix_binding_stays_armed() {
    let mut state = repeat_state();
    assert_eq!(
        state.config.prefix_repeat_timeout,
        std::time::Duration::from_millis(750)
    );

    assert!(state.handle_input_bytes(&[0x02]).actions.is_empty());
    let before = std::time::Instant::now();
    let next = state.handle_input_bytes(b"n");
    let after = std::time::Instant::now();

    assert_eq!(tab_focus_targets(&next), vec!["tab_2".to_owned()]);
    assert_eq!(state.mode, ClientShellMode::Prefix);
    let deadline = state.prefix_repeat_deadline.expect("armed deadline");
    assert!(deadline >= before + std::time::Duration::from_millis(750));
    assert!(deadline <= after + std::time::Duration::from_millis(750));
}

#[test]
fn armed_bare_key_repeats_and_rearms() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"n");
    let first = state.prefix_repeat_deadline.expect("armed deadline");

    // Bare `n` (no prefix) while armed repeats the action and re-arms.
    let repeat = state.handle_input_bytes(b"n");

    assert_eq!(tab_focus_targets(&repeat), vec!["tab_2".to_owned()]);
    assert!(
        repeat.requests.is_empty(),
        "the key must not reach the pane"
    );
    assert_eq!(state.mode, ClientShellMode::Prefix);
    let second = state.prefix_repeat_deadline.expect("re-armed deadline");
    assert!(second >= first);
}

#[test]
fn non_repeatable_prefix_binding_exits_and_clears() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    let previous = state.handle_input_bytes(b"p");

    assert_eq!(tab_focus_targets(&previous), vec!["tab_2".to_owned()]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn default_prefix_binding_does_not_arm() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(surface());

    state.handle_input_bytes(&[0x02]);
    let next = state.handle_input_bytes(b"n");

    assert_eq!(tab_focus_targets(&next), vec!["tab_2".to_owned()]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn esc_while_armed_exits_and_clears() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"n");
    assert_eq!(state.mode, ClientShellMode::Prefix);

    let escape = state.handle_input_bytes(&[0x1b]);

    assert!(escape.actions.is_empty());
    assert!(escape.requests.is_empty());
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn unbound_key_while_armed_exits_and_clears() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"n");
    assert_eq!(state.mode, ClientShellMode::Prefix);

    let unbound = state.handle_input_bytes(b"a");

    assert!(unbound.actions.is_empty());
    assert!(unbound.requests.is_empty());
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn prefix_key_while_armed_sends_prefix_and_clears() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"n");
    assert_eq!(state.mode, ClientShellMode::Prefix);

    let literal_prefix = state.handle_input_bytes(&[0x02]);

    assert!(
        !literal_prefix.requests.is_empty(),
        "the second prefix press reaches the focused pane"
    );
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn repeatable_mode_changing_action_does_not_rearm() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"r");

    assert_eq!(state.mode, ClientShellMode::Resize);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn repeatable_overlay_action_does_not_rearm() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"?");

    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn prefix_repeat_expiry_leaves_prefix() {
    let mut state = repeat_state();

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"n");
    let deadline = state.prefix_repeat_deadline.expect("armed deadline");

    assert!(!state.tick_prefix_repeat(deadline - std::time::Duration::from_millis(1)));
    assert_eq!(state.mode, ClientShellMode::Prefix);
    assert!(state.prefix_repeat_deadline.is_some());

    assert!(state.tick_prefix_repeat(deadline));
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.prefix_repeat_deadline.is_none());
}

#[test]
fn timer_delay_includes_prefix_repeat_deadline() {
    let mut state = repeat_state();
    let now = std::time::Instant::now();
    state.prefix_repeat_deadline = Some(now + std::time::Duration::from_millis(30));

    assert_eq!(state.timer_delay(now), std::time::Duration::from_millis(30));
}

use super::*;

#[test]
fn pasted_help_and_copy_queries_strip_control_characters() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.overlay = Some(ClientShellOverlay::Help(ClientHelpOverlay {
        query: String::new(),
        search_focused: true,
        scroll: 0,
    }));

    assert!(state.insert_overlay_text("work\nspace"));
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Help(ClientHelpOverlay { ref query, .. }))
            if query == "workspace"
    ));

    state.overlay = None;
    state.copy_mode = Some(ClientCopyModeState {
        pane_id: "pane_1".into(),
        popup: false,
        content_revision: 0,
        geometry: (80, 24),
        cursor: crate::api::schema::PaneTextPoint { row: 0, col: 0 },
        offset_from_bottom: 0,
        max_offset_from_bottom: 0,
        entry_offset_from_bottom: 0,
        selection: None,
        search_prompt: Some(ClientCopySearchPrompt {
            direction: crate::api::schema::PaneCopySearchDirection::Forward,
            query: String::new(),
        }),
        search_query: String::new(),
        search_direction: None,
        search_matches: Vec::new(),
        search_total: 0,
        search_current: None,
        search_current_global: None,
        search_generation: 0,
        copy_after_search: false,
    });

    assert!(state.insert_copy_search_text("needle\r\n"));
    assert_eq!(
        state
            .copy_mode
            .as_ref()
            .and_then(|copy_mode| copy_mode.search_prompt.as_ref())
            .map(|prompt| prompt.query.as_str()),
        Some("needle")
    );
}

#[test]
fn client_mouse_selection_highlights_and_copies_through_endpoint_extraction() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();

    let down = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pane.inner_rect.x,
        row: pane.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &down.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneFocus(target) if target.pane_id == "pane_1"
            )
    ));
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| !selection.is_visible()));

    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: pane.inner_rect.x + 2,
        row: pane.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(drag.repaint);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));
    let selected = state.compose(106, 20).expect("selected frame");
    let selected_cell =
        &selected.cells[usize::from(pane.inner_rect.y) * 106 + usize::from(pane.inner_rect.x)];
    assert_ne!(
        selected_cell.bg,
        crate::protocol::color_to_u32(ratatui::style::Color::Reset)
    );

    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: pane.inner_rect.x + 2,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(state.selection.is_none());
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("selection release should request endpoint extraction");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneSelectionRead(params)
            if params.pane_id == "pane_1"
                && params.anchor == crate::api::schema::PaneTextPoint { row: 0, col: 0 }
                && params.cursor == crate::api::schema::PaneTextPoint { row: 0, col: 2 }
    ));

    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneSelection {
            pane_id: "pane_1".into(),
            text: "LIV".into(),
        }),
    );
    assert!(repaint);
    assert!(matches!(
        &actions[..],
        [ClientShellAction::ClipboardWrite(bytes)] if bytes == b"LIV"
    ));
    assert_eq!(
        state
            .copy_feedback
            .as_ref()
            .map(|feedback| feedback.message.as_str()),
        Some("copied to clipboard")
    );
}

#[test]
fn clipboard_feedback_is_client_local_and_respects_config() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    let now = std::time::Instant::now();
    assert!(state.show_copy_feedback(now));
    assert_eq!(
        state
            .copy_feedback
            .as_ref()
            .map(|feedback| feedback.message.as_str()),
        Some("copied to clipboard")
    );
    assert_eq!(
        state.copy_feedback_deadline,
        Some(now + std::time::Duration::from_secs(2))
    );

    state.config.clipboard_toast_enabled = false;
    state.copy_feedback = None;
    state.copy_feedback_deadline = None;
    assert!(!state.show_copy_feedback(now));
    assert!(state.copy_feedback.is_none());
    assert!(state.copy_feedback_deadline.is_none());
}

#[test]
fn retained_mouse_selection_survives_output_and_copies_without_terminal_input() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.config.copy_on_select = false;
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();
    for event in [
        crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: pane.inner_rect.x,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        },
        crossterm::event::MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: pane.inner_rect.x + 2,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        },
        crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: pane.inner_rect.x + 2,
            row: pane.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        },
    ] {
        state.handle_raw_events(vec![RawInputEvent::Mouse(event)]);
        // Output can arrive between drag and release, including an in-flight revision.
        let mut updated = state.pane_surface.clone().expect("pane surface");
        updated.panes[0].content_revision += 1;
        updated.frame.cells[0].symbol = "x".into();
        state.set_pane_surface(updated);
    }
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_finalized));

    // A patch that redraws selected text must retain the same live terminal range.
    let mut updated = state.pane_surface.clone().expect("pane surface");
    updated.panes[0].content_revision += 1;
    let mut cell = updated.frame.cells[0].clone();
    cell.symbol = "y".into();
    assert!(matches!(
        state.apply_pane_surface_patch(crate::protocol::PaneSurfacePatch {
            boot_id: updated.boot_id,
            projection_revision: updated.projection_revision,
            base_surface_revision: updated.surface_revision,
            surface_revision: updated.surface_revision + 1,
            panes: updated.panes,
            rows: vec![crate::protocol::PaneSurfacePatchRow {
                x: 0,
                y: 0,
                cells: vec![cell]
            }],
            cursor: updated.frame.cursor,
        }),
        super::super::surface_patch::ClientPaneSurfacePatchOutcome::Applied(_)
    ));
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_finalized));

    let highlighted = state.compose(106, 20).expect("highlighted frame");
    let cell_index = usize::from(pane.inner_rect.y) * 106 + usize::from(pane.inner_rect.x);
    let selected_cell = highlighted.cells[cell_index].clone();
    let selection = state.selection.take();
    let unselected = state.compose(106, 20).expect("unselected frame");
    assert_ne!(selected_cell.bg, unselected.cells[cell_index].bg);
    state.selection = selection;

    let copy = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ))]);
    assert!(state.selection.is_none());
    assert!(matches!(
        &copy.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(request.method, crate::api::schema::Method::PaneSelectionRead(
                crate::api::schema::PaneSelectionReadParams { content_revision: None, .. }
            ))
    ));
    assert!(copy.requests.is_empty());
    let request_id = match &copy.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneSelection {
            pane_id: "pane_1".into(),
            text: "yIV".into(),
        }),
    );
    assert!(matches!(&actions[..], [ClientShellAction::ClipboardWrite(bytes)] if bytes == b"yIV"));
}

#[test]
fn selection_edge_drag_requests_scroll_and_timer_continues_it() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 20,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let pane = state.hits.panes[0].clone();
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: pane.inner_rect.x,
        row: pane.inner_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: pane.inner_rect.x,
        row: pane.inner_rect.y.saturating_sub(1),
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(matches!(
        &drag.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 3
            )
    ));
    let drag_request_id = match &drag.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    let now = std::time::Instant::now();
    state.selection_autoscroll_deadline = Some(now);
    let tick = state.tick_selection_autoscroll(now);
    assert!(tick.actions.is_empty());
    let (_, next_scroll) =
        state.handle_endpoint_result("boot-1", &drag_request_id, Ok(pane_scroll_result(3, 20, 3)));
    assert!(matches!(
        &next_scroll[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 4
            )
    ));
}

#[test]
fn keyboard_copy_mode_owns_cursor_selection_copy_and_scroll_restore() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.config.copy_on_select = false;
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 20,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");

    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.row),
        Some(21)
    );
    assert!(enter.actions.is_empty());

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('b'),
        KeyModifiers::CONTROL,
    ))]);
    assert_eq!(state.mode, ClientShellMode::Prefix);
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Copy);

    let page = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::PageUp,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.row),
        Some(20)
    );
    assert!(matches!(
        &page.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 1
            )
    ));
    let page_request_id = match &page.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };

    let top = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('g'),
        KeyModifiers::empty(),
    ))]);
    assert!(top.actions.is_empty());
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.row),
        Some(0)
    );
    let (_, top_actions) =
        state.handle_endpoint_result("boot-1", &page_request_id, Ok(pane_scroll_result(1, 20, 2)));
    let [ClientShellAction::Endpoint { request, .. }] = &top_actions[..] else {
        panic!("latest queued scroll should follow the completed request");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneScroll(params)
            if params.pane_id == "pane_1" && params.offset_from_bottom == 20
    ));
    let top_request_id = request.id.clone();
    state.handle_endpoint_result("boot-1", &top_request_id, Ok(pane_scroll_result(20, 20, 2)));

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('v'),
        KeyModifiers::empty(),
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('l'),
        KeyModifiers::empty(),
    ))]);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));

    let copy = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('y'),
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state.copy_mode.is_none());
    assert!(state.selection.is_none());
    assert_eq!(copy.actions.len(), 2);
    assert!(copy.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(&request.method, crate::api::schema::Method::PaneSelectionRead(params)
                if params.content_revision == Some(0))
    )));
    assert!(copy.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 0
            )
    )));
}

#[test]
fn keyboard_copy_mode_content_motion_is_endpoint_backed_and_stale_safe() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 0,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    let origin = state.copy_mode.as_ref().expect("copy mode").cursor;

    let motion = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('w'),
        KeyModifiers::empty(),
    ))]);
    let [ClientShellAction::Endpoint { request, .. }] = &motion.actions[..] else {
        panic!("word motion should use endpoint semantics");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneCopyMotion(params)
            if params.cursor == origin
                && params.motion == crate::api::schema::PaneCopyMotion::NextWordStart
    ));
    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneCopyMotion {
            pane_id: "pane_1".into(),
            cursor: crate::api::schema::PaneTextPoint {
                row: origin.row,
                col: 3,
            },
            content_revision: 0,
        }),
    );
    assert!(repaint);
    assert!(actions.is_empty());
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.col),
        Some(3)
    );
}

#[test]
fn copy_search_owns_prompt_repeat_highlights_selection_and_restore() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 20,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    let origin = state.copy_mode.as_ref().expect("copy mode").cursor;

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('?'),
        KeyModifiers::SHIFT,
    ))]);
    assert!(state.copy_mode.as_ref().is_some_and(|mode| {
        mode.search_prompt.as_ref().is_some_and(|prompt| {
            prompt.direction == crate::api::schema::PaneCopySearchDirection::Backward
        })
    }));
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|mode| mode.search_prompt.is_none()));

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('/'),
        KeyModifiers::empty(),
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Text(crate::input::TextCommit::new(
        "junk",
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Text(crate::input::TextCommit::new(
        "nee",
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Paste("dleX".into())]);
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Backspace,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(
        state
            .copy_mode
            .as_ref()
            .and_then(|mode| mode.search_prompt.as_ref())
            .map(|prompt| prompt.query.as_str()),
        Some("needle")
    );

    let search = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Enter,
        KeyModifiers::empty(),
    ))]);
    let [ClientShellAction::Endpoint { request, .. }] = &search.actions[..] else {
        panic!("search should use endpoint terminal semantics");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneCopySearch(params)
            if params.pane_id == "pane_1"
                && params.query == "needle"
                && params.direction == crate::api::schema::PaneCopySearchDirection::Forward
                && params.cursor == origin
                && params.previous.is_none()
    ));
    let matches = vec![
        crate::api::schema::PaneTextRange {
            start: crate::api::schema::PaneTextPoint { row: 5, col: 2 },
            end: crate::api::schema::PaneTextPoint { row: 5, col: 7 },
        },
        crate::api::schema::PaneTextRange {
            start: crate::api::schema::PaneTextPoint { row: 15, col: 1 },
            end: crate::api::schema::PaneTextPoint { row: 15, col: 6 },
        },
    ];
    let (repaint, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(copy_search_result(matches.clone(), Some(0))),
    );
    assert!(repaint);
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.row),
        Some(5)
    );
    assert!(actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 15
            )
    )));
    let initial_scroll_id = actions
        .iter()
        .find_map(|action| match action {
            ClientShellAction::Endpoint { request, .. }
                if matches!(request.method, crate::api::schema::Method::PaneScroll(_)) =>
            {
                Some(request.id.clone())
            }
            _ => None,
        })
        .expect("initial search scroll");
    state.handle_endpoint_result(
        "boot-1",
        &initial_scroll_id,
        Ok(pane_scroll_result(15, 20, 2)),
    );
    let mut scrolled_surface = state.pane_surface.clone().expect("pane surface");
    scrolled_surface.panes[0]
        .scroll
        .as_mut()
        .expect("scroll metrics")
        .offset_from_bottom = 15;
    state.set_pane_surface(scrolled_surface);
    let frame = state.compose(106, 20).expect("search frame");
    let hit = state.hits.panes[0].clone();
    let viewport_top = 5u16;
    let restored = frame.to_ratatui_buffer().expect("search frame buffer");
    let highlighted = restored
        .cell((hit.inner_rect.x + 2, hit.inner_rect.y + (5 - viewport_top)))
        .expect("highlighted search cell");
    assert_eq!(highlighted.bg, state.config.palette.accent);

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('v'),
        KeyModifiers::empty(),
    ))]);
    let repeat = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('n'),
        KeyModifiers::empty(),
    ))]);
    let [ClientShellAction::Endpoint { request, .. }] = &repeat.actions[..] else {
        panic!("repeat should use endpoint search");
    };
    let repeat_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneCopySearch(params)
            if params.direction == crate::api::schema::PaneCopySearchDirection::Forward
                && params.previous == Some(matches[0])
    ));
    let (_, repeat_actions) = state.handle_endpoint_result(
        "boot-1",
        &repeat_id,
        Ok(copy_search_result(matches.clone(), Some(1))),
    );
    if let Some(scroll_id) = repeat_actions.iter().find_map(|action| match action {
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::PaneScroll(_)) =>
        {
            Some(request.id.clone())
        }
        _ => None,
    }) {
        state.handle_endpoint_result("boot-1", &scroll_id, Ok(pane_scroll_result(6, 20, 2)));
    }
    assert_eq!(
        state.copy_mode.as_ref().map(|mode| mode.cursor.row),
        Some(15)
    );
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));

    let reverse = state.handle_raw_events(vec![RawInputEvent::Key(
        crate::input::TerminalKey::new(KeyCode::Char('N'), KeyModifiers::SHIFT),
    )]);
    let [ClientShellAction::Endpoint { request, .. }] = &reverse.actions[..] else {
        panic!("reverse search should use endpoint search");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneCopySearch(params)
            if params.direction == crate::api::schema::PaneCopySearchDirection::Backward
                && params.previous == Some(matches[1])
    ));
    let (_, reverse_actions) = state.handle_endpoint_result(
        "boot-1",
        &request.id,
        Ok(copy_search_result(matches.clone(), Some(0))),
    );
    if let Some(scroll_id) = reverse_actions.iter().find_map(|action| match action {
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::PaneScroll(_)) =>
        {
            Some(request.id.clone())
        }
        _ => None,
    }) {
        state.handle_endpoint_result("boot-1", &scroll_id, Ok(pane_scroll_result(15, 20, 2)));
    }

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|mode| mode.search_query.is_empty() && mode.selection.is_none()));
    let exit = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(exit.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneScroll(params)
                    if params.offset_from_bottom == 0
            )
    )));
}

#[test]
fn navigator_renders_connected_siblings_and_ancestor_lines() {
    let mut snapshot = snapshot();
    snapshot.focused_pane_id = None;
    snapshot.tabs[0].label = "editor".into();
    snapshot.panes[0].label = Some("agent".into());
    snapshot.panes[0].focused = false;
    let mut shell = snapshot.panes[0].clone();
    shell.pane_id = "pane_shell".into();
    shell.label = Some("shell".into());
    snapshot.panes.push(shell);
    for label in ["notes", "logs"] {
        let mut tab = snapshot.tabs[0].clone();
        tab.tab_id = format!("tab_{label}");
        tab.label = label.into();
        tab.focused = false;
        tab.number = snapshot.tabs.len() + 1;
        let mut pane = snapshot.panes[0].clone();
        pane.pane_id = format!("pane_{label}");
        pane.tab_id = tab.tab_id.clone();
        pane.label = Some(label.into());
        pane.focused = false;
        snapshot.tabs.push(tab);
        snapshot.panes.push(pane);
    }
    let mut workspace = snapshot.workspaces[0].clone();
    workspace.workspace_id = "ws_2".into();
    workspace.active_tab_id = "tab_last".into();
    workspace.label = "second".into();
    workspace.number = 2;
    workspace.focused = false;
    let mut tab = snapshot.tabs[0].clone();
    tab.workspace_id = workspace.workspace_id.clone();
    tab.tab_id = workspace.active_tab_id.clone();
    tab.label = "last".into();
    tab.focused = false;
    let mut pane = snapshot.panes[0].clone();
    pane.workspace_id = workspace.workspace_id.clone();
    pane.tab_id = tab.tab_id.clone();
    pane.pane_id = "pane_last".into();
    snapshot.workspaces.push(workspace);
    snapshot.tabs.push(tab);
    snapshot.panes.push(pane);
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot));
    state.set_pane_surface(surface());
    state.open_navigator_overlay();
    let prefixes = |state: &mut ClientShellState, height| {
        let frame = state.compose(106, height).expect("navigator frame");
        state
            .hits
            .navigator_rows
            .iter()
            .map(|(rect, _)| {
                frame.cells[rect.y as usize * frame.width as usize + rect.x as usize + 1..]
                    .iter()
                    .take(6)
                    .map(|cell| cell.symbol.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        prefixes(&mut state, 30),
        [
            "▾ clie",
            "├── ed",
            "│  ├──",
            "│  └──",
            "├── no",
            "│  └──",
            "└── lo",
            "   └──",
            "▾ seco",
            "└── la",
            "   └──"
        ]
    );

    // The editor ancestor is above this viewport; the logs sibling is below it.
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_mut() else {
        panic!("expected navigator");
    };
    navigator.scroll = 2;
    navigator.selected = Some(ClientNavigatorTarget::Pane {
        endpoint_id: state.active_endpoint_id.clone(),
        pane_id: "pane_shell".into(),
    });
    assert_eq!(
        prefixes(&mut state, 12),
        ["│  ├──", "│  └──", "├── no", "│  └──"]
    );

    // Excluded siblings must not leave dangling continuation lines.
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_mut() else {
        panic!("expected navigator");
    };
    navigator.query = "shell".into();
    navigator.scroll = 0;
    assert_eq!(prefixes(&mut state, 30), ["▾ clie", "└── ed", "   └──"]);
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_mut() else {
        panic!("expected navigator");
    };
    navigator.query.clear();
    navigator.expanded_workspaces.clear();
    assert_eq!(prefixes(&mut state, 30), ["▸ clie", "▸ seco"]);
}

#[test]
#[ignore = "manual navigator composition scaling profile"]
fn navigator_render_scale_profile() {
    for panes in [1, 15, 52] {
        let mut snapshot = snapshot();
        for index in 1..panes {
            let mut pane = snapshot.panes[0].clone();
            pane.pane_id = format!("pane_{index}_extra");
            pane.focused = false;
            snapshot.panes.push(pane);
        }
        let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
        state.set_snapshot(Box::new(snapshot));
        state.set_pane_surface(surface());
        state.open_navigator_overlay();
        for _ in 0..20 {
            std::hint::black_box(state.compose(106, 30).expect("navigator frame"));
        }
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            std::hint::black_box(state.compose(106, 30).expect("navigator frame"));
        }
        eprintln!(
            "navigator: {panes} panes, {:.1} us/frame",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[test]
fn navigator_owns_search_mouse_selection_and_stable_target_focus() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let mut open = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::OpenNavigator),
        &mut open,
    );
    let navigator = state.compose(106, 30).expect("navigator overlay");
    let navigator_text = navigator
        .cells
        .chunks(navigator.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(navigator_text.contains("client-shell"));
    assert!(navigator_text.contains("pane 1"));

    let search = state.hits.navigator_search;
    let focus_search =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: search.x,
            row: search.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(focus_search.repaint);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Navigator(ClientNavigatorOverlay {
            search_focused: true,
            ..
        }))
    ));
    assert!(state.handle_input_bytes(b"client").actions.is_empty());
    let filtered = state.compose(106, 30).expect("filtered navigator");
    assert!(filtered
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.visible));

    state.handle_input_bytes(b"\x1b");
    state.handle_input_bytes(b"a");
    state.compose(106, 30).expect("navigator rows");
    let pane_target = {
        let ClientShellOverlay::Navigator(navigator) = state.overlay.as_ref().expect("navigator")
        else {
            panic!("expected navigator");
        };
        render::client_navigator_rows(&state.endpoints, &state.active_endpoint_id, navigator)
            .iter()
            .find(|row| matches!(row.target, ClientNavigatorTarget::Pane { .. }))
            .map(|row| row.target.clone())
            .expect("pane row")
    };
    let pane_rect = state
        .hits
        .navigator_rows
        .iter()
        .find(|(_, target)| *target == pane_target)
        .map(|(rect, _)| *rect)
        .expect("visible pane row");
    let select =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Moved,
            column: pane_rect.x + 6,
            row: pane_rect.y,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(select.repaint);
    let accept =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: pane_rect.x + 6,
            row: pane_rect.y,
            modifiers: KeyModifiers::empty(),
        })]);
    let [ClientShellAction::Endpoint { request, .. }] = &accept.actions[..] else {
        panic!("navigator pane click should use endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneFocus(target) if target.pane_id == "pane_1"
    ));
    assert!(state.overlay.is_none());
}

#[test]
fn copy_mode_survives_mouse_motion_and_parks_across_focus_changes() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.config.copy_on_select = false;
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 10,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Moved,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    })]);
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state.copy_mode.is_some());

    state.handle_input_bytes(b"v");
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|copy_mode| copy_mode.selection.is_some()));

    let mut unfocused = snapshot();
    unfocused.focused_pane_id = Some("pane_2".into());
    unfocused.panes[0].focused = false;
    unfocused.panes.push(ClientShellPane {
        pane_id: "pane_2".into(),
        workspace_id: "ws_1".into(),
        tab_id: "tab_1".into(),
        label: None,
        cwd: Some("/repo".into()),
        foreground_cwd: Some("/repo".into()),
        focused: true,
        right_click_passthrough: false,
    });
    state.set_snapshot(Box::new(unfocused.clone()));
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|copy_mode| copy_mode.selection.is_some()));

    let (prefix_key, prefix_modifiers) = state.config.keybinds.prefix;
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        prefix_key,
        prefix_modifiers,
    ))]);
    state.set_snapshot(Box::new(unfocused.clone()));
    assert_eq!(state.mode, ClientShellMode::Prefix);
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Terminal);

    let mut other_selection =
        crate::selection::Selection::absolute_range("pane_2".to_owned(), (0, 0), (0, 1));
    assert!(other_selection.finish());
    state.selection = Some(other_selection);
    state.set_snapshot(Box::new(unfocused));
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "pane_2"));

    let mut other_surface = surface();
    other_surface.panes[0].pane_id = "pane_2".into();
    state.set_pane_surface(other_surface.clone());
    other_surface.surface_revision += 1;
    other_surface.panes[0].content_revision = 1;
    state.set_pane_surface(other_surface);
    assert!(state.selection.is_some());
    let copy = state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ))]);
    assert!(copy.requests.is_empty());
    assert!(
        matches!(&copy.actions[..], [ClientShellAction::Endpoint { request, .. }]
        if matches!(&request.method, crate::api::schema::Method::PaneSelectionRead(params)
            if params.pane_id == "pane_2" && params.content_revision.is_none()))
    );

    state.set_snapshot(Box::new(snapshot()));
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state.copy_mode.is_some());
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "pane_1"));
    state.handle_raw_events(vec![RawInputEvent::Text(crate::input::TextCommit::new(
        "ignored",
    ))]);
    state.handle_raw_events(vec![RawInputEvent::Paste("ignored".into())]);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "pane_1"));

    state.mode = ClientShellMode::Navigate;
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "pane_1"));
    state.mode = ClientShellMode::Resize;
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        KeyCode::Esc,
        KeyModifiers::empty(),
    ))]);
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "pane_1"));
}

#[test]
fn retained_selection_copy_suppresses_key_repeats() {
    let mut config = Config::default();
    config.ui.copy_on_select = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let mut selection =
        crate::selection::Selection::absolute_range("pane_1".to_owned(), (0, 0), (0, 1));
    assert!(selection.finish());
    state.selection = Some(selection);

    let key = crate::input::TerminalKey::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let press = state.handle_raw_events(vec![RawInputEvent::Key(key.clone())]);
    assert!(press.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::PaneSelectionRead(_))
    )));
    let repeat = state.handle_raw_events(vec![RawInputEvent::Key(
        key.clone()
            .with_kind(crossterm::event::KeyEventKind::Repeat),
    )]);
    assert!(repeat.actions.is_empty());
    assert!(repeat.requests.is_empty());
    let release = state.handle_raw_events(vec![RawInputEvent::Key(
        key.with_kind(crossterm::event::KeyEventKind::Release),
    )]);
    assert!(release.actions.is_empty());
    assert!(release.requests.is_empty());
}

#[test]
fn rapid_copy_motions_are_chained_from_the_previous_result() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 0,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    let origin = state.copy_mode.as_ref().expect("copy mode").cursor;

    let first = state.handle_input_bytes(b"w");
    let second = state.handle_input_bytes(b"w");
    assert_eq!(first.actions.len(), 1);
    assert!(second.actions.is_empty());
    let first_id = match &first.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    let intermediate = crate::api::schema::PaneTextPoint {
        row: origin.row,
        col: 2,
    };
    let (_, follow_up) = state.handle_endpoint_result(
        "boot-1",
        &first_id,
        Ok(crate::api::schema::ResponseResult::PaneCopyMotion {
            pane_id: "pane_1".into(),
            cursor: intermediate,
            content_revision: 0,
        }),
    );
    assert!(matches!(
        &follow_up[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneCopyMotion(params)
                    if params.cursor == intermediate
            )
    ));
}

#[test]
fn queued_copy_keys_preserve_prefix_order() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 10,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    let origin = state.copy_mode.as_ref().expect("copy mode").cursor;
    let motion = state.handle_input_bytes(b"w");
    state.handle_input_bytes(b"l");
    let (prefix_key, prefix_modifiers) = state.config.keybinds.prefix;
    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        prefix_key,
        prefix_modifiers,
    ))]);
    let motion_id = match &motion.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    state.handle_endpoint_result(
        "boot-1",
        &motion_id,
        Ok(crate::api::schema::ResponseResult::PaneCopyMotion {
            pane_id: "pane_1".into(),
            cursor: origin,
            content_revision: 0,
        }),
    );
    assert_eq!(state.mode, ClientShellMode::Prefix);
    assert_eq!(
        state
            .copy_mode
            .as_ref()
            .map(|copy_mode| copy_mode.cursor.col),
        Some(origin.col.saturating_add(1))
    );
}

#[test]
fn reentering_copy_mode_on_the_same_pane_is_a_no_op() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 10,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut first = ClientShellInput::default();
    assert!(state.enter_copy_mode(&mut first));
    state
        .copy_mode
        .as_mut()
        .expect("copy mode")
        .offset_from_bottom = 10;
    let mut reenter = ClientShellInput::default();
    assert!(state.enter_copy_mode(&mut reenter));
    assert!(reenter.actions.is_empty());
    assert_eq!(
        state
            .copy_mode
            .as_ref()
            .map(|copy_mode| copy_mode.entry_offset_from_bottom),
        Some(0)
    );
}

#[test]
fn copy_waits_for_endpoint_motion_before_copying_selection() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 0,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface);
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    state.handle_input_bytes(b"v");
    let origin = state.copy_mode.as_ref().expect("copy mode").cursor;
    let motion = state.handle_input_bytes(b"w");
    let queued_copy = state.handle_input_bytes(b"y");
    assert!(queued_copy.actions.is_empty());
    let motion_id = match &motion.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    let target = crate::api::schema::PaneTextPoint {
        row: origin.row,
        col: 2,
    };
    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &motion_id,
        Ok(crate::api::schema::ResponseResult::PaneCopyMotion {
            pane_id: "pane_1".into(),
            cursor: target,
            content_revision: 0,
        }),
    );
    assert_eq!(state.mode, ClientShellMode::Terminal);
    assert!(actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::PaneSelectionRead(params)
                    if params.anchor == origin && params.cursor == target
            )
    )));
}

#[test]
fn new_content_revision_invalidates_copy_search_coordinates() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    let mut pane_surface = surface();
    pane_surface.panes[0].scroll = Some(crate::protocol::PaneSurfaceScrollMetrics {
        offset_from_bottom: 0,
        max_offset_from_bottom: 0,
        viewport_rows: 2,
    });
    state.set_pane_surface(pane_surface.clone());
    state.compose(106, 20).expect("composed frame");
    let mut enter = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::CopyMode),
        &mut enter,
    );
    let copy_mode = state.copy_mode.as_mut().expect("copy mode");
    copy_mode.search_query = "needle".into();
    copy_mode
        .search_matches
        .push(crate::api::schema::PaneTextRange {
            start: crate::api::schema::PaneTextPoint { row: 0, col: 0 },
            end: crate::api::schema::PaneTextPoint { row: 0, col: 1 },
        });
    copy_mode.search_total = 1;
    copy_mode.search_current = Some(0);
    copy_mode.search_current_global = Some(0);

    pane_surface.panes[0].content_revision = 2;
    state.set_pane_surface(pane_surface);
    let copy_mode = state.copy_mode.as_ref().expect("copy mode retained");
    assert!(copy_mode.search_matches.is_empty());
    assert_eq!(copy_mode.search_total, 0);
    assert_eq!(copy_mode.search_current, None);
}

#[test]
fn word_selection_result_survives_focus_snapshot_lag() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("composed frame");
    let hit = state.hits.panes[0].clone();
    let mut request = ClientShellInput::default();
    state.request_word_selection(&hit, 0, 1, &mut request);
    let request_id = match &request.actions[0] {
        ClientShellAction::Endpoint { request, .. } => request.id.clone(),
        _ => unreachable!(),
    };
    let mut lagging = snapshot();
    lagging.focused_pane_id = None;
    lagging.panes[0].focused = false;
    state.set_snapshot(Box::new(lagging));
    let (repaint, _) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PaneSelection {
            pane_id: "pane_1".into(),
            text: "hello world".into(),
        }),
    );
    assert!(repaint);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));
}

#[test]
fn popup_selection_copies_through_endpoint_extraction() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();
    assert!(popup.scroll.is_some(), "v2 metrics should reach the hit");

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(|selection| selection.pane_id == "terminal-popup"));

    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 4,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(drag.repaint);
    let selected = state.compose(106, 20).expect("selected popup frame");
    let selected_cell =
        &selected.cells[usize::from(popup.inner_rect.y) * 106 + usize::from(popup.inner_rect.x)];
    assert_ne!(
        selected_cell.bg,
        crate::protocol::color_to_u32(ratatui::style::Color::Reset),
        "the popup selection must be painted over the blitted popup cells"
    );

    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: popup.inner_rect.x + 4,
            row: popup.inner_rect.y,
            modifiers: KeyModifiers::empty(),
        })]);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("popup selection release should request endpoint extraction");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PopupSelectionRead(params)
            if params.terminal_id == "terminal-popup"
                && params.anchor == crate::api::schema::PaneTextPoint { row: 0, col: 0 }
                && params.cursor == crate::api::schema::PaneTextPoint { row: 0, col: 4 }
    ));

    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PopupSelection {
            terminal_id: "terminal-popup".into(),
            text: "popup".into(),
        }),
    );
    assert!(matches!(
        &actions[..],
        [ClientShellAction::ClipboardWrite(bytes)] if bytes == b"popup"
    ));
}

#[test]
fn popup_drag_selection_uses_absolute_rows_when_scrolled_back() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    // Scrolled two lines back in a three-row popup: the top visible row is
    // absolute row 5 of an eight-row history.
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 2, 7, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x,
        row: popup.inner_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 3,
        row: popup.inner_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    let release =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: popup.inner_rect.x + 3,
            row: popup.inner_rect.y + 1,
            modifiers: KeyModifiers::empty(),
        })]);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("popup selection release should request endpoint extraction");
    };
    assert!(
        matches!(
            &request.method,
            crate::api::schema::Method::PopupSelectionRead(params)
                if params.anchor.row == 6 && params.cursor.row == 6
        ),
        "scrolled-back popup rows must be absolute, got {:?}",
        request.method
    );
}

#[test]
fn popup_copy_motion_targets_the_popup_terminal() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_popup());
    state.compose(106, 20).expect("popup frame");

    let mut outcome = ClientShellInput::default();
    assert!(state.enter_copy_mode(&mut outcome));
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|copy_mode| copy_mode.popup && copy_mode.pane_id == "terminal-popup"));

    let motion = state.handle_input_bytes(b"$");
    let [ClientShellAction::Endpoint { request, .. }] = &motion.actions[..] else {
        panic!("copy motion should reach the endpoint");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PopupCopyMotion(params)
            if params.terminal_id == "terminal-popup"
    ));
}

#[test]
fn popup_copy_mode_is_unavailable_without_v2_surface_metrics() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface_with_popup());
    state.compose(106, 20).expect("popup frame");

    let mut outcome = ClientShellInput::default();
    assert!(
        !state.enter_copy_mode(&mut outcome),
        "a v1 server sends no popup scroll metrics, so copy mode must stay closed"
    );
    assert!(state.copy_mode.is_none());
}

/// Selects across one popup row and releases, returning the release outcome.
fn drag_popup_row(
    state: &mut ClientShellState,
    popup: &PaneHit,
    viewport_row: u16,
) -> ClientShellInput {
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let column = if matches!(kind, MouseEventKind::Down(_)) {
            popup.inner_rect.x
        } else {
            popup.inner_rect.x + 3
        };
        let outcome =
            state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
                kind,
                column,
                row: popup.inner_rect.y + viewport_row,
                modifiers: KeyModifiers::empty(),
            })]);
        if matches!(kind, MouseEventKind::Up(_)) {
            return outcome;
        }
    }
    unreachable!("the release arm returns")
}

/// A popup scroll that leaves the rendered popup identical still moves every
/// visible row to a different absolute scrollback row, and it does not touch
/// `content_revision`, so the endpoint's staleness guard cannot catch a
/// mismatch. The client must therefore never resolve a selection through
/// metrics belonging to a frame other than the one on screen.
#[test]
fn popup_metrics_from_another_frame_never_resolve_a_selection() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 2, 7, 3));
    state.set_pane_surface(selectable_popup_surface_at_revision(1));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();
    assert_eq!(
        popup.scroll.map(|scroll| scroll.offset_from_bottom),
        Some(2)
    );

    // The server has scrolled on and reported it, but the frame that move
    // produced has not been applied yet. The staged report must not reach the
    // hit the user is currently looking at.
    state.apply_popup_surface_metrics(&popup_surface_metrics(2, 5, 7, 3));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();
    assert_eq!(
        popup.scroll.map(|scroll| scroll.offset_from_bottom),
        Some(2),
        "metrics stamped for a later frame must not retarget the displayed one"
    );

    let release = drag_popup_row(&mut state, &popup, 1);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("popup selection release should request endpoint extraction");
    };
    assert!(
        matches!(
            &request.method,
            crate::api::schema::Method::PopupSelectionRead(params)
                if params.anchor.row == 6 && params.cursor.row == 6
        ),
        "rows must follow the displayed frame, got {:?}",
        request.method
    );

    // Applying that frame commits its own metrics, and only then do the rows move.
    state.set_pane_surface(selectable_popup_surface_at_revision(2));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();
    assert_eq!(
        popup.scroll.map(|scroll| scroll.offset_from_bottom),
        Some(5)
    );
    let release = drag_popup_row(&mut state, &popup, 1);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("popup selection release should request endpoint extraction");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PopupSelectionRead(params)
            if params.anchor.row == 3 && params.cursor.row == 3
    ));
}

#[test]
fn a_popup_frame_without_matching_metrics_disables_selection() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    // A report stamped for a frame that never arrives leaves the displayed frame
    // without metrics; selection must be off rather than guess an offset.
    state.apply_popup_surface_metrics(&popup_surface_metrics(9, 2, 7, 3));
    state.set_pane_surface(selectable_popup_surface_at_revision(1));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();
    assert!(popup.scroll.is_none());

    let release = drag_popup_row(&mut state, &popup, 1);
    assert!(state.selection.is_none());
    assert!(
        release.actions.is_empty(),
        "no selection may be extracted without metrics for the displayed frame"
    );

    let mut outcome = ClientShellInput::default();
    assert!(!state.enter_copy_mode(&mut outcome));
}

#[test]
fn popup_drag_autoscroll_scrolls_the_popup_terminal() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 2, 7, 3));
    state.set_pane_surface(selectable_popup_surface_at_revision(1));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x + 1,
        row: popup.inner_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    // Dragging above the popup's first row arms the autoscroll.
    let drag = state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 1,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(drag.repaint);
    let deadline = state
        .selection_autoscroll_deadline
        .expect("popup drag should arm autoscroll");

    let tick = state.tick_selection_autoscroll(deadline);
    let scrolled = tick.actions.iter().any(|action| {
        matches!(
            action,
            ClientShellAction::Endpoint { request, .. }
                if matches!(
                    &request.method,
                    crate::api::schema::Method::PopupScroll(params)
                        if params.terminal_id == "terminal-popup"
                )
        )
    });
    assert!(
        scrolled,
        "popup autoscroll must scroll the popup terminal, got {:?}",
        tick.actions.len()
    );
}

/// Snapshots arrive on any agent status change, and the popup terminal is never
/// one of their panes. A plain membership test therefore pruned the popup's
/// pending scroll state on every snapshot, which pinned copy mode's offset.
#[test]
fn a_popup_scroll_target_survives_an_agent_status_snapshot() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 2, 7, 3));
    state.set_pane_surface(surface_with_popup());
    state.compose(106, 20).expect("popup frame");

    let mut outcome = ClientShellInput::default();
    assert!(state.enter_copy_mode(&mut outcome));
    state.push_pane_scroll_offset("terminal-popup".into(), 5, &mut outcome);
    assert!(state.pane_scroll_targets.contains_key("terminal-popup"));
    assert!(state.pane_scroll_in_flight.contains_key("terminal-popup"));

    // An unrelated agent finishes; the server re-projects and the client gets a
    // fresh snapshot. Nothing about the popup changed.
    let mut next = snapshot();
    next.revision = 2;
    if let Some(agent) = next.agents.first_mut() {
        agent.agent_status = crate::api::schema::AgentStatus::Done;
    }
    state.set_snapshot(Box::new(next));

    assert!(
        state.pane_scroll_targets.contains_key("terminal-popup"),
        "a snapshot must not drop the popup's pending scroll target"
    );
    assert!(state.pane_scroll_in_flight.contains_key("terminal-popup"));
    assert_eq!(state.mode, ClientShellMode::Copy);
    assert!(state
        .copy_mode
        .as_ref()
        .is_some_and(|copy_mode| copy_mode.popup));
}

/// The popup's reached scroll target is not in `surface.panes`, so it needs the
/// same clearing panes get. Left behind it pins the copy-mode offset and
/// suppresses the copy cursor and highlights permanently.
#[test]
fn a_reached_popup_scroll_target_is_cleared() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 2, 7, 3));
    state.set_pane_surface(selectable_popup_surface_at_revision(1));

    let mut outcome = ClientShellInput::default();
    state.push_pane_scroll_offset("terminal-popup".into(), 5, &mut outcome);
    assert!(state.pane_scroll_targets.contains_key("terminal-popup"));

    // The server reports the popup sitting at the requested offset.
    state.apply_popup_surface_metrics(&popup_surface_metrics(2, 5, 7, 3));
    state.set_pane_surface(selectable_popup_surface_at_revision(2));

    assert!(
        !state.pane_scroll_targets.contains_key("terminal-popup"),
        "a reached popup scroll target must be cleared"
    );
}

/// With `copy_on_select` off a popup selection is retained just like a pane's,
/// so ctrl+c has to reach the shell instead of interrupting the popup process.
#[test]
fn the_copy_shortcut_reaches_the_shell_over_a_retained_popup_selection() {
    let mut config = Config::default();
    config.ui.copy_on_select = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    drag_popup_row(&mut state, &popup, 0);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));

    let copy = state.handle_input_bytes(b"\x03");
    assert!(
        copy.requests.is_empty(),
        "ctrl+c must not reach the popup process, got {:?}",
        copy.requests
    );
    let [ClientShellAction::Endpoint { request, .. }] = &copy.actions[..] else {
        panic!("ctrl+c should extract the retained popup selection");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PopupSelectionRead(params)
            if params.terminal_id == "terminal-popup"
    ));
}

/// A double click in the popup sends `popup.selection.read`, but the completion
/// path dropped any reply whose target was absent from `snapshot.panes` — which
/// the popup terminal always is.
#[test]
fn popup_word_selection_completes() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    let mut request = ClientShellInput::default();
    state.request_word_selection(&popup, 0, 1, &mut request);
    let [ClientShellAction::Endpoint { request, .. }] = &request.actions[..] else {
        panic!("a popup word selection should reach the endpoint");
    };
    let request_id = request.id.clone();
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PopupSelectionRead(params)
            if params.terminal_id == "terminal-popup"
    ));

    let (repaint, _) = state.handle_endpoint_result(
        "boot-1",
        &request_id,
        Ok(crate::api::schema::ResponseResult::PopupSelection {
            terminal_id: "terminal-popup".into(),
            text: "popup-live".into(),
        }),
    );
    assert!(repaint);
    assert!(
        state
            .selection
            .as_ref()
            .is_some_and(crate::selection::Selection::is_visible),
        "the word selection reply must land instead of being discarded"
    );
}

/// A popup word-selection reply lands now, so an in-flight one must not arrive
/// after the user has started a fresh drag and replace it with the word range.
#[test]
fn a_new_popup_selection_invalidates_a_pending_word_selection() {
    // The selection has to survive the release for a late reply to clobber it.
    let mut config = Config::default();
    config.ui.copy_on_select = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    let mut request = ClientShellInput::default();
    state.request_word_selection(&popup, 0, 1, &mut request);
    let [ClientShellAction::Endpoint { request, .. }] = &request.actions[..] else {
        panic!("a popup word selection should reach the endpoint");
    };
    let stale_id = request.id.clone();
    assert!(state.pending_word_selection.is_some());

    // The user gives up on the double click and drags a new selection instead.
    // With copy-on-select off it stays retained on the screen.
    drag_popup_row(&mut state, &popup, 0);
    let dragged = state
        .selection
        .as_ref()
        .map(|selection| selection.ordered_cells())
        .expect("the drag selection");

    let (_, actions) = state.handle_endpoint_result(
        "boot-1",
        &stale_id,
        Ok(crate::api::schema::ResponseResult::PopupSelection {
            terminal_id: "terminal-popup".into(),
            text: "popup-live".into(),
        }),
    );
    assert!(actions.is_empty());
    assert_eq!(
        state
            .selection
            .as_ref()
            .map(|selection| selection.ordered_cells()),
        Some(dragged),
        "a stale word-selection reply must not replace the drag selection"
    );
}

/// A mouse selection in the popup has no `copy_mode` to be restored from, so
/// the snapshot cleanup has to exempt the popup on its own account.
#[test]
fn a_popup_mouse_selection_survives_an_agent_status_snapshot() {
    let mut config = Config::default();
    config.ui.copy_on_select = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    drag_popup_row(&mut state, &popup, 0);
    let dragged = state
        .selection
        .as_ref()
        .map(|selection| selection.ordered_cells())
        .expect("the drag selection");
    assert!(state.copy_mode.is_none(), "a mouse drag opens no copy mode");

    let mut next = snapshot();
    next.revision = 2;
    if let Some(agent) = next.agents.first_mut() {
        agent.agent_status = crate::api::schema::AgentStatus::Done;
    }
    state.set_snapshot(Box::new(next));

    assert_eq!(
        state
            .selection
            .as_ref()
            .map(|selection| selection.ordered_cells()),
        Some(dragged),
        "an unrelated snapshot must not wipe the popup's mouse selection"
    );
}

/// Typing into the popup ends a retained selection the way it does over a pane.
/// Otherwise the stale range stays visible and the copy shortcut copies it.
#[test]
fn typing_into_the_popup_ends_a_retained_selection() {
    let mut config = Config::default();
    config.ui.copy_on_select = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    drag_popup_row(&mut state, &popup, 0);
    assert!(state
        .selection
        .as_ref()
        .is_some_and(crate::selection::Selection::is_visible));

    // An ordinary key reaches the popup and clears the selection on the way.
    let typed = state.handle_input_bytes(b"x");
    assert!(matches!(
        &typed.requests[..],
        [ClientMessage::ClientShellPopupInput { terminal_id, .. }]
            if terminal_id == "terminal-popup"
    ));
    assert!(
        state.selection.is_none(),
        "the popup key must end the retained selection"
    );

    // So the copy shortcut no longer has a stale range to copy.
    let copy = state.handle_input_bytes(b"\x03");
    assert!(
        copy.actions.is_empty(),
        "nothing is selected, so ctrl+c must not extract anything"
    );
    assert!(matches!(
        &copy.requests[..],
        [ClientMessage::ClientShellPopupInput { terminal_id, .. }]
            if terminal_id == "terminal-popup"
    ));
}

/// A wheel during a popup drag has to carry the selection's end along with the
/// viewport; otherwise releasing copies the range from before the scroll.
#[test]
fn a_wheel_during_a_popup_drag_moves_the_selection_end() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 4, 7, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 2,
        row: popup.inner_rect.y + 1,
        modifiers: KeyModifiers::empty(),
    })]);
    let before = state
        .selection
        .as_ref()
        .map(|selection| selection.ordered_cells())
        .expect("the drag selection");

    let scrolled =
        state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: popup.inner_rect.x + 2,
            row: popup.inner_rect.y + 1,
            modifiers: KeyModifiers::empty(),
        })]);
    assert!(
        scrolled.actions.iter().any(|action| matches!(
            action,
            ClientShellAction::Endpoint { request, .. }
                if matches!(&request.method, crate::api::schema::Method::PopupScroll(_))
        )),
        "the wheel must scroll the popup terminal, got {:?}",
        scrolled.actions.len()
    );
    assert!(
        scrolled
            .requests
            .iter()
            .all(|request| !matches!(request, ClientMessage::ClientShellPopupInput { .. })),
        "a wheel during a drag drives the selection, not the popup process"
    );
    let after = state
        .selection
        .as_ref()
        .map(|selection| selection.ordered_cells())
        .expect("the selection survives the scroll");
    assert_ne!(
        before, after,
        "the selection end must follow the scrolled viewport"
    );
}

/// A popup that resizes under an in-progress mouse selection invalidates it:
/// the anchored cells no longer mean what they did.
#[test]
fn a_popup_resize_invalidates_a_mouse_selection() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(surface_with_selectable_popup());
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 3,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(state.selection.is_some());

    // The popup comes back a different size on the next frame.
    let mut resized = selectable_popup_surface_at_revision(2);
    if let Some(popup_surface) = resized.popup.as_deref_mut() {
        let narrower = Buffer::with_lines(["popup", "", ""]);
        popup_surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&narrower, None, &[]);
    }
    state.apply_popup_surface_metrics(&popup_surface_metrics(2, 0, 0, 3));
    state.set_pane_surface(resized);

    assert!(
        state.selection.is_none(),
        "a resized popup must drop the selection anchored to the old geometry"
    );
}

/// Builds a popup surface whose visible rows read `lines`.
fn popup_surface_with_lines(surface_revision: u64, lines: [&str; 3]) -> PaneSurfaceFrame {
    let mut surface = selectable_popup_surface_at_revision(surface_revision);
    if let Some(popup) = surface.popup.as_deref_mut() {
        let buffer = Buffer::with_lines(lines);
        popup.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
    }
    surface
}

/// Sets up a popup drag selection over the first row and returns the state.
fn popup_state_with_drag_selection() -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.apply_popup_surface_metrics(&popup_surface_metrics(1, 0, 0, 3));
    state.set_pane_surface(popup_surface_with_lines(1, ["keep-me", "noise-a", ""]));
    state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit").clone();

    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: popup.inner_rect.x,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: popup.inner_rect.x + 6,
        row: popup.inner_rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    assert!(
        state.selection.is_some(),
        "the drag should start a selection"
    );
    state
}

/// A popup writes constantly, so output that leaves the selected cells alone
/// must not cancel a drag the user is still making.
#[test]
fn popup_output_outside_the_selection_keeps_the_drag() {
    let mut state = popup_state_with_drag_selection();
    assert!(state.config.copy_on_select, "the default path is the risk");

    // Row 2 changes; the selected row 0 does not.
    let mut metrics = popup_surface_metrics(2, 0, 0, 3);
    metrics.content_revision = 2;
    state.apply_popup_surface_metrics(&metrics);
    state.set_pane_surface(popup_surface_with_lines(2, ["keep-me", "noise-b", ""]));

    assert!(
        state.selection.is_some(),
        "output outside the selection must not cancel the drag"
    );
}

/// But a write that lands on the selected cells does invalidate it: those rows
/// no longer say what the user picked.
#[test]
fn popup_output_inside_the_selection_drops_it() {
    let mut state = popup_state_with_drag_selection();

    let mut metrics = popup_surface_metrics(2, 0, 0, 3);
    metrics.content_revision = 2;
    state.apply_popup_surface_metrics(&metrics);
    state.set_pane_surface(popup_surface_with_lines(2, ["changed", "noise-a", ""]));

    assert!(
        state.selection.is_none(),
        "a write over the selected cells must drop the selection"
    );
}

/// Switching to the alternate screen replaces the buffer without necessarily
/// resizing the popup, so the selection has to go with it.
#[test]
fn a_popup_screen_switch_drops_the_selection() {
    let mut state = popup_state_with_drag_selection();

    // Same size, same revision, same cells — only the screen changed.
    let mut metrics = popup_surface_metrics(2, 0, 0, 3);
    metrics.alternate_screen_active = true;
    state.apply_popup_surface_metrics(&metrics);
    state.set_pane_surface(popup_surface_with_lines(2, ["keep-me", "noise-a", ""]));

    assert!(
        state.selection.is_none(),
        "an alternate screen switch must drop the selection"
    );
}

use super::*;

fn minimal_config(align: TabBarAlign, title: bool) -> Config {
    let mut config = Config::default();
    config.ui.tab_bar_style = TabBarStyle::Minimal;
    config.ui.tab_bar_align = align;
    config.ui.tab_bar_title = title;
    config
}

fn tabs_snapshot(count: usize) -> ClientShellSnapshot {
    let mut projected = snapshot();
    for number in 2..=count {
        let mut tab = projected.tabs[0].clone();
        tab.tab_id = format!("tab_{number}");
        tab.number = number;
        tab.label = number.to_string();
        tab.focused = false;
        projected.tabs.push(tab);
    }
    projected
}

fn bar_text(frame: &FrameData, bar: Rect) -> String {
    let row = bar.y as usize * frame.width as usize;
    frame.cells[row + bar.x as usize..row + bar.right() as usize]
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect()
}

fn cell_at(frame: &FrameData, rect: Rect) -> &crate::protocol::CellData {
    &frame.cells[rect.y as usize * frame.width as usize + rect.x as usize]
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> RawInputEvent {
    RawInputEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

#[test]
fn minimal_tab_bar_right_aligns_glyph_slots_without_chrome_buttons() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        false,
    )));
    state.set_snapshot(Box::new(tabs_snapshot(3)));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("minimal tab bar");
    let bar = state.layout(106, 20).tab_bar;

    let rects = state
        .hits
        .tabs
        .iter()
        .map(|(rect, _)| *rect)
        .collect::<Vec<_>>();
    assert_eq!(rects.len(), 3);
    assert!(rects.iter().all(|rect| rect.width == 2));
    for pair in rects.windows(2) {
        assert_eq!(pair[0].right(), pair[1].x);
    }
    assert_eq!(rects[2].right(), bar.right());
    assert_eq!(state.tab_scroll, 0);
    assert_eq!(state.hits.new_tab, Rect::default());
    assert_eq!(state.hits.tab_scroll_left, Rect::default());
    assert_eq!(state.hits.tab_scroll_right, Rect::default());
}

#[test]
fn minimal_tab_bar_left_aligns_from_the_bar_start() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Left,
        false,
    )));
    state.set_snapshot(Box::new(tabs_snapshot(3)));
    state.set_pane_surface(surface());
    state
        .compose(106, 20)
        .expect("left aligned minimal tab bar");
    let bar = state.layout(106, 20).tab_bar;

    assert_eq!(state.hits.tabs[0].0.x, bar.x);
    assert_eq!(state.hits.tabs[2].0.right(), bar.x + 6);
}

#[test]
fn minimal_tab_bar_colors_the_focused_marker_with_the_accent() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        false,
    )));
    state.set_snapshot(Box::new(tabs_snapshot(2)));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("minimal markers");
    let palette = &state.config.palette;

    let active = cell_at(&frame, state.hits.tabs[0].0);
    assert_eq!(active.symbol, "\u{F09DE}");
    assert_eq!(active.fg, crate::protocol::color_to_u32(palette.accent));
    assert_eq!(active.bg, crate::protocol::color_to_u32(palette.panel_bg));
    assert_eq!(active.modifier, 0);

    let inactive = cell_at(&frame, state.hits.tabs[1].0);
    assert_eq!(inactive.symbol, "\u{EABC}");
    assert_eq!(inactive.fg, crate::protocol::color_to_u32(palette.overlay1));
    assert_eq!(inactive.bg, crate::protocol::color_to_u32(palette.panel_bg));
    assert_eq!(inactive.modifier, 0);
}

#[test]
fn minimal_glyph_only_slots_hide_labels_and_the_zoom_marker() {
    let mut projected = tabs_snapshot(1);
    projected.tabs[0].label = "logs".into();
    projected.tabs[0].zoomed = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        false,
    )));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("glyph-only minimal tab bar");
    let bar = state.layout(106, 20).tab_bar;

    let row = bar_text(&frame, bar);
    assert!(
        !row.contains("logs"),
        "glyph-only strip drew a label: {row:?}"
    );
    assert!(!row.contains('Z'), "glyph-only strip drew zoom: {row:?}");
    assert_eq!(state.hits.tabs[0].0.width, 2);
}

#[test]
fn minimal_titled_slots_render_the_label_and_zoom_marker() {
    let mut projected = tabs_snapshot(1);
    projected.tabs[0].label = "logs".into();
    projected.tabs[0].zoomed = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        true,
    )));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("titled minimal tab bar");
    let bar = state.layout(106, 20).tab_bar;

    let row = bar_text(&frame, bar);
    assert!(
        row.contains("\u{F09DE} logs Z"),
        "titled strip should draw the glyph and label: {row:?}"
    );
    assert_eq!(state.hits.tabs[0].0.width, 9);
    assert_eq!(state.hits.tabs[0].0.right(), bar.right());
}

#[test]
fn minimal_tab_bar_overflow_packs_from_the_left_and_clamps_scroll() {
    let mut config = ClientShellConfig::from_config(&minimal_config(TabBarAlign::Right, false));
    config.mobile_width_threshold = 0;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(tabs_snapshot(40)));
    state.set_pane_surface(surface());
    state.compose(60, 20).expect("overflowing minimal tab bar");
    let bar = state.layout(60, 20).tab_bar;

    assert!(state.hits.tabs.len() < 40);
    assert_eq!(state.hits.tabs[0].0.x, bar.x);
    assert!(state.hits.tabs.iter().any(|(_, tab_id)| tab_id == "tab_1"));

    state.tab_scroll = usize::MAX;
    state.reveal_focused_tab = false;
    state.compose(60, 20).expect("clamped minimal tab scroll");
    assert!(state.tab_scroll < 40);
    assert_eq!(state.hits.tabs[0].0.x, bar.x);
    assert_eq!(state.hits.tabs.last().expect("visible tab").1, "tab_40");

    state
        .compose(300, 20)
        .expect("minimal tab bar without overflow");
    assert_eq!(state.tab_scroll, 0);
    assert_eq!(state.hits.tabs.len(), 40);
}

#[test]
fn minimal_tab_bar_right_aligns_against_the_status_segments() {
    let mut projected = tabs_snapshot(3);
    projected.tab_bar_right = vec![crate::protocol::ClientShellTabStatusSegment {
        text: "ZOOM".into(),
        accent: true,
    }];
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        false,
    )));
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("minimal tab bar with status");
    let bar = state.layout(106, 20).tab_bar;

    assert!(bar_text(&frame, bar).ends_with("ZOOM"));
    assert_eq!(
        state.hits.tabs.last().expect("last tab").0.right(),
        bar.right() - 5
    );
}

#[test]
fn minimal_tab_bar_click_wheel_and_drag_reorder_by_stable_id() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&minimal_config(
        TabBarAlign::Right,
        false,
    )));
    state.set_snapshot(Box::new(tabs_snapshot(3)));
    state.set_pane_surface(surface());
    state.compose(106, 20).expect("three minimal tabs");
    let first = state.hits.tabs[0].0;
    let second = state.hits.tabs[1].0;
    let third = state.hits.tabs[2].0;

    state.handle_raw_events(vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        second.x,
        second.y,
    )]);
    let click = state.handle_raw_events(vec![mouse(
        MouseEventKind::Up(MouseButton::Left),
        second.x,
        second.y,
    )]);
    assert!(matches!(
        &click.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"
            )
    ));

    let wheel = state.handle_raw_events(vec![mouse(MouseEventKind::ScrollDown, first.x, first.y)]);
    assert!(matches!(
        &wheel.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"
            )
    ));
    assert_eq!(state.tab_scroll, 0);

    state.handle_raw_events(vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        first.x,
        first.y,
    )]);
    let drag = state.handle_raw_events(vec![mouse(
        MouseEventKind::Drag(MouseButton::Left),
        third.right().saturating_sub(1),
        third.y,
    )]);
    assert!(drag.repaint);
    assert!(matches!(
        state.chrome_drag,
        Some(ClientChromeDrag::Tab {
            ref tab_id,
            insert_index: Some(3),
            ..
        }) if tab_id == "tab_1"
    ));
    let frame = state.compose(106, 20).expect("minimal drop indicator");
    let bar = state.layout(106, 20).tab_bar;
    assert!(bar_text(&frame, bar).contains('│'));

    let release = state.handle_raw_events(vec![mouse(
        MouseEventKind::Up(MouseButton::Left),
        third.right().saturating_sub(1),
        third.y,
    )]);
    let [ClientShellAction::Endpoint { request, .. }] = &release.actions[..] else {
        panic!("minimal tab drag should use the endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::TabMove(params)
            if params.tab_id == "tab_1" && params.insert_index == 3
    ));
}

#[test]
fn minimal_tab_bar_drag_during_overflow_targets_a_visible_marker() {
    let mut config = ClientShellConfig::from_config(&minimal_config(TabBarAlign::Right, false));
    config.mobile_width_threshold = 0;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(tabs_snapshot(40)));
    state.set_pane_surface(surface());
    state.compose(60, 20).expect("overflowing minimal tab bar");
    let first = state.hits.tabs[0].0;
    let fifth = state.hits.tabs[4].0;

    state.handle_raw_events(vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        first.x,
        first.y,
    )]);
    state.handle_raw_events(vec![mouse(
        MouseEventKind::Drag(MouseButton::Left),
        fifth.x,
        fifth.y,
    )]);
    assert!(matches!(
        state.chrome_drag,
        Some(ClientChromeDrag::Tab {
            ref tab_id,
            insert_index: Some(4),
            ..
        }) if tab_id == "tab_1"
    ));
}

#[test]
fn minimal_tab_bar_keeps_hits_without_mouse_chrome() {
    let mut config = minimal_config(TabBarAlign::Right, false);
    config.ui.mouse_capture = false;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(tabs_snapshot(2)));
    state.set_pane_surface(surface());
    state
        .compose(106, 20)
        .expect("minimal tab bar without mouse chrome");
    let bar = state.layout(106, 20).tab_bar;

    assert_eq!(state.hits.tabs.len(), 2);
    assert_eq!(state.hits.tabs[1].0.right(), bar.right());
    let tab = state.hits.tabs[1].0;
    let wheel = state.handle_raw_events(vec![mouse(MouseEventKind::ScrollDown, tab.x, tab.y)]);
    assert!(matches!(
        &wheel.actions[..],
        [ClientShellAction::Endpoint { request, .. }]
            if matches!(
                &request.method,
                crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"
            )
    ));
}

#[test]
fn classic_tab_bar_ignores_the_minimal_alignment_and_title_keys() {
    let mut config = Config::default();
    config.ui.tab_bar_align = TabBarAlign::Right;
    config.ui.tab_bar_title = true;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(tabs_snapshot(2)));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("classic tab bar");
    let bar = state.layout(106, 20).tab_bar;

    assert_eq!(state.hits.tabs[0].0.x, bar.x);
    assert!(state.hits.tabs[0].0.width >= MIN_TAB_WIDTH);
    assert!(state.hits.new_tab.width > 0);
    let row = bar_text(&frame, bar);
    assert!(!row.contains('\u{F09DE}'), "classic drew a marker: {row:?}");
    assert!(!row.contains('\u{EABC}'), "classic drew a marker: {row:?}");
}

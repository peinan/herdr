use super::*;

fn highlight_config() -> Config {
    toml::from_str(
        r#"
[ui]
prefix_indicator = "highlight"
"#,
    )
    .expect("prefix highlight config")
}

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

/// A 13x4 two-pane surface coloured the way the server sends it: focused pane A
/// owns the accent ring, including the seam column it shares with pane B, and
/// pane B keeps the unfocused border colour. Mirrors `line_touches_pane`.
fn bordered_surface() -> PaneSurfaceFrame {
    let palette = crate::app::client_palette_from_config(&Config::default());
    let mut buffer = Buffer::with_lines([
        "┌────┐┌─────┐",
        "│    ││     │",
        "│    ││     │",
        "└────┘└─────┘",
    ]);
    for x in 0..=6u16 {
        for y in [0u16, 3] {
            buffer[(x, y)].set_fg(palette.accent);
        }
    }
    for y in 0..=3u16 {
        for x in [0u16, 5, 6] {
            buffer[(x, y)].set_fg(palette.accent);
        }
    }
    for x in 7..=12u16 {
        for y in [0u16, 3] {
            buffer[(x, y)].set_fg(palette.overlay0);
        }
    }
    for y in 1..=2u16 {
        buffer[(12, y)].set_fg(palette.overlay0);
    }
    for (offset, symbol) in ["▌", "A"].into_iter().enumerate() {
        buffer[(1 + offset as u16, 0)].set_symbol(symbol).set_style(
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        );
    }
    // Accent-coloured pane content the border pass must not touch.
    buffer[(1, 1)].set_symbol("x").set_fg(palette.accent);

    PaneSurfaceFrame {
        boot_id: "boot-1".into(),
        projection_revision: 1,
        surface_revision: 1,
        frame: FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]),
        panes: vec![
            PaneSurfacePane {
                pane_id: "pane_1".into(),
                content_revision: 0,
                rect: SurfaceRect {
                    x: 0,
                    y: 0,
                    width: 6,
                    height: 4,
                },
                inner_rect: SurfaceRect {
                    x: 1,
                    y: 1,
                    width: 4,
                    height: 2,
                },
                scrollbar_rect: None,
                scroll: None,
                focused: true,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            },
            PaneSurfacePane {
                pane_id: "pane_2".into(),
                content_revision: 0,
                rect: SurfaceRect {
                    x: 6,
                    y: 0,
                    width: 7,
                    height: 4,
                },
                inner_rect: SurfaceRect {
                    x: 7,
                    y: 1,
                    width: 5,
                    height: 2,
                },
                scrollbar_rect: None,
                scroll: None,
                focused: false,
                mouse_reporting: false,
                sgr_pixel_mouse: false,
                alternate_screen_active: false,
                pixel_width: 0,
                pixel_height: 0,
            },
        ],
        splits: Vec::new(),
        popup: None,
        graphics: crate::protocol::SurfaceGraphicsScene::default(),
    }
}

fn frame_text(frame: &FrameData) -> String {
    frame
        .cells
        .chunks(usize::from(frame.width))
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn surface_cell(frame: &FrameData, origin: Rect, x: u16, y: u16) -> &crate::protocol::CellData {
    let index = usize::from(origin.y + y) * usize::from(frame.width) + usize::from(origin.x + x);
    &frame.cells[index]
}

fn tab_background(frame: &FrameData, hits: &ShellHitMap, tab_id: &str) -> u32 {
    let (rect, _) = hits
        .tabs
        .iter()
        .find(|(_, id)| id == tab_id)
        .unwrap_or_else(|| panic!("hit for {tab_id}"));
    frame.cells[usize::from(rect.y) * usize::from(frame.width) + usize::from(rect.x)].bg
}

#[test]
fn prefix_highlight_recolors_the_focused_border_instead_of_drawing_the_hint_bar() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&highlight_config()));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(bordered_surface());
    let palette = state.config.palette.clone();
    let accent = crate::protocol::color_to_u32(palette.accent);
    let yellow = crate::protocol::color_to_u32(palette.yellow);
    let overlay0 = crate::protocol::color_to_u32(palette.overlay0);
    let origin = state.layout(106, 20).pane_surface;

    let terminal = state.compose(106, 20).expect("terminal frame");
    assert_eq!(surface_cell(&terminal, origin, 0, 0).fg, accent);
    assert_eq!(
        tab_background(&terminal, &state.hits, "tab_1"),
        accent,
        "the active tab keeps the accent outside prefix mode"
    );

    assert!(state.handle_input_bytes(&[0x02]).repaint);
    let frame = state.compose(106, 20).expect("prefix frame");
    let text = frame_text(&frame);
    assert!(!text.contains("PREFIX"), "frame: {text:?}");

    for (x, y, label) in [
        (0, 0, "top-left corner"),
        (5, 0, "top-right corner"),
        (6, 0, "seam corner"),
        (0, 2, "left edge"),
        (6, 2, "seam column"),
        (3, 3, "bottom edge"),
        (6, 3, "seam bottom corner"),
    ] {
        assert_eq!(
            surface_cell(&frame, origin, x, y).fg,
            yellow,
            "{label} at ({x}, {y})"
        );
    }
    for (x, y, label) in [
        (12, 0, "neighbour top-right corner"),
        (12, 2, "neighbour right edge"),
        (8, 3, "neighbour bottom edge"),
    ] {
        assert_eq!(
            surface_cell(&frame, origin, x, y).fg,
            overlay0,
            "{label} at ({x}, {y})"
        );
    }

    let title = surface_cell(&frame, origin, 2, 0);
    assert_eq!(title.symbol, "A");
    assert_eq!(title.fg, yellow);
    assert_ne!(
        title.modifier, 0,
        "the recolor keeps the title's bold modifier"
    );
    let decoy = surface_cell(&frame, origin, 1, 1);
    assert_eq!(decoy.symbol, "x");
    assert_eq!(decoy.fg, accent, "pane content keeps its own accent");

    assert_eq!(tab_background(&frame, &state.hits, "tab_1"), yellow);
    assert_eq!(
        tab_background(&frame, &state.hits, "tab_2"),
        crate::protocol::color_to_u32(palette.surface0),
        "inactive tabs are unaffected"
    );
}

#[test]
fn prefix_status_bar_default_keeps_the_hint_bar_and_the_accent_chrome() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(bordered_surface());
    let accent = crate::protocol::color_to_u32(state.config.palette.accent);
    let origin = state.layout(106, 20).pane_surface;

    assert!(state.handle_input_bytes(&[0x02]).repaint);
    let frame = state.compose(106, 20).expect("prefix frame");
    let text = frame_text(&frame);
    assert!(text.contains("PREFIX"), "frame: {text:?}");
    assert_eq!(surface_cell(&frame, origin, 0, 0).fg, accent);
    assert_eq!(surface_cell(&frame, origin, 6, 2).fg, accent);
    assert_eq!(tab_background(&frame, &state.hits, "tab_1"), accent);
}

#[test]
fn endpoint_errors_keep_their_bar_while_prefix_highlight_recolors_the_border() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&highlight_config()));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(bordered_surface());
    let yellow = crate::protocol::color_to_u32(state.config.palette.yellow);
    let origin = state.layout(106, 20).pane_surface;
    assert!(state.handle_input_bytes(&[0x02]).repaint);
    // Input dismisses a pending endpoint error, so raise it once prefix is armed.
    state.endpoint_error = Some("endpoint rejected the request".to_owned());
    let frame = state
        .compose(106, 20)
        .expect("prefix frame with endpoint error");
    let text = frame_text(&frame);
    assert!(text.contains("ERROR"), "frame: {text:?}");
    assert!(
        text.contains("endpoint rejected the request"),
        "frame: {text:?}"
    );
    assert!(!text.contains("PREFIX"), "frame: {text:?}");
    assert_eq!(surface_cell(&frame, origin, 0, 0).fg, yellow);
}

#[test]
fn leaving_prefix_restores_the_accent_border_and_tab() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&highlight_config()));
    state.set_snapshot(Box::new(two_tab_snapshot()));
    state.set_pane_surface(bordered_surface());
    let accent = crate::protocol::color_to_u32(state.config.palette.accent);
    let origin = state.layout(106, 20).pane_surface;

    state.handle_input_bytes(&[0x02]);
    state.compose(106, 20).expect("prefix frame");
    state.handle_input_bytes(&[0x1b]);
    assert_eq!(state.mode, ClientShellMode::Terminal);

    let frame = state.compose(106, 20).expect("restored frame");
    for (x, y) in [(0, 0), (6, 0), (0, 2), (6, 2), (3, 3)] {
        assert_eq!(
            surface_cell(&frame, origin, x, y).fg,
            accent,
            "border at ({x}, {y})"
        );
    }
    assert_eq!(tab_background(&frame, &state.hits, "tab_1"), accent);
}

#[test]
fn bottom_tab_bar_keeps_its_hits_and_status_accent_under_prefix_highlight() {
    let mut snapshot = two_tab_snapshot();
    snapshot.tab_bar_right = vec![crate::protocol::ClientShellTabStatusSegment {
        text: "ZOOM".into(),
        accent: true,
    }];
    let mut config = highlight_config();
    config.ui.tab_bar_position = TabBarPositionConfig::Bottom;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot.clone()));
    state.set_pane_surface(bordered_surface());
    let accent = crate::protocol::color_to_u32(state.config.palette.accent);
    let tab_bar = state.layout(106, 20).tab_bar;

    assert!(state.handle_input_bytes(&[0x02]).repaint);
    let frame = state.compose(106, 20).expect("bottom tab bar frame");
    assert!(
        state.hits.tabs.iter().any(|(_, id)| id == "tab_1"),
        "the suppressed hint bar leaves the bottom tab row clickable"
    );
    let status_x = tab_bar.right() - 4;
    for offset in 0..4 {
        let index = usize::from(tab_bar.y) * usize::from(frame.width)
            + usize::from(status_x + offset as u16);
        assert_eq!(
            frame.cells[index].bg, accent,
            "tab_bar_right stays accent at offset {offset}"
        );
    }

    let mut status_bar = ClientShellState::new(ClientShellConfig::from_config(&{
        let mut config = Config::default();
        config.ui.tab_bar_position = TabBarPositionConfig::Bottom;
        config
    }));
    status_bar.set_snapshot(Box::new(snapshot));
    status_bar.set_pane_surface(bordered_surface());
    status_bar.handle_input_bytes(&[0x02]);
    status_bar.compose(106, 20).expect("bottom hint bar frame");
    assert!(
        status_bar.hits.tabs.is_empty(),
        "the hint bar owns the bottom row in status_bar mode"
    );
}

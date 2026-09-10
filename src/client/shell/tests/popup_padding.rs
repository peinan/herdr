use super::*;

/// A client configured with `[ui] popup_padding = { top = 1, left = 2 }`.
fn padded_client_config() -> ClientShellConfig {
    let config: Config = toml::from_str(
        r#"
[ui]
popup_padding = { top = 1, left = 2 }
"#,
    )
    .expect("config");
    ClientShellConfig::from_config(&config)
}

/// The popup fixture with a terminal frame of exactly `cols` x `rows`, which
/// is what tells the client whether the server resolved the same padding.
fn surface_with_popup_frame(cols: u16, rows: u16) -> PaneSurfaceFrame {
    let mut surface = surface_with_popup();
    let popup_buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
    if let Some(popup) = surface.popup.as_deref_mut() {
        popup.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&popup_buffer, None, &[]);
    }
    surface
}

fn composed_popup_hit(config: ClientShellConfig, surface: PaneSurfaceFrame) -> PaneHit {
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface);
    state.compose(106, 20).expect("popup frame");
    state
        .hits
        .popup
        .as_ref()
        .expect("popup hit geometry")
        .clone()
}

#[test]
fn the_default_config_leaves_the_popup_terminal_area_flush_with_the_border() {
    let popup = composed_popup_hit(
        ClientShellConfig::from_config(&Config::default()),
        surface_with_popup(),
    );

    assert_eq!(popup.inner_rect.x, popup.rect.x + 1);
    assert_eq!(popup.inner_rect.y, popup.rect.y + 1);
    assert_eq!(popup.inner_rect.width, 9);
    assert_eq!(popup.inner_rect.height, 3);
}

#[test]
fn popup_padding_moves_the_terminal_area_inside_the_border() {
    // The server resolved the same padding, so it ships a 7x2 frame: 10 inner
    // columns less 2 of padding less the spacer column, and 3 inner rows less
    // one of padding.
    let popup = composed_popup_hit(padded_client_config(), surface_with_popup_frame(7, 2));

    assert_eq!(popup.rect.width, 12);
    assert_eq!(popup.rect.height, 5);
    assert_eq!(popup.inner_rect.x, popup.rect.x + 3);
    assert_eq!(popup.inner_rect.y, popup.rect.y + 2);
    assert_eq!(popup.inner_rect.width, 7);
    assert_eq!(popup.inner_rect.height, 2);
}

#[test]
fn a_server_that_did_not_apply_this_padding_keeps_the_unpadded_area() {
    // A remote server with no popup_padding of its own still sizes the popup
    // PTY to the full 9x3 terminal area. Blitting that into a padded rect
    // would silently drop its right and bottom edges, so the client drops the
    // padding instead.
    let popup = composed_popup_hit(padded_client_config(), surface_with_popup_frame(9, 3));

    assert_eq!(popup.inner_rect.x, popup.rect.x + 1);
    assert_eq!(popup.inner_rect.y, popup.rect.y + 1);
    assert_eq!(popup.inner_rect.width, 9);
    assert_eq!(popup.inner_rect.height, 3);
}

#[test]
fn a_frame_size_still_in_flight_keeps_the_padded_area() {
    // Mid-resize the frame matches neither geometry. Falling back here would
    // blink the padding off for a frame, so the padded area stands.
    let popup = composed_popup_hit(padded_client_config(), surface_with_popup_frame(5, 2));

    assert_eq!(popup.inner_rect.x, popup.rect.x + 3);
    assert_eq!(popup.inner_rect.width, 7);
}

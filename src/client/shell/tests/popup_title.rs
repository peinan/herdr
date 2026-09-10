use super::*;

/// The composed popup's top border row, as the terminal would show it.
fn popup_top_border(title: &str) -> String {
    let mut surface = surface_with_popup();
    if let Some(popup) = surface.popup.as_deref_mut() {
        popup.title = title.into();
    }
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface);

    let frame = state.compose(106, 20).expect("composed frame");
    let rect = state.hits.popup.as_ref().expect("popup hit geometry").rect;
    (rect.x..rect.x + rect.width)
        .map(|x| {
            frame.cells[rect.y as usize * frame.width as usize + x as usize]
                .symbol
                .as_str()
        })
        .collect()
}

#[test]
fn the_popup_title_is_flanked_by_spaces() {
    // The fixture popup is 12 cells wide: two corners, the padded label, and
    // border fill for the rest.
    assert_eq!(popup_top_border("rp"), "┌ rp ──────┐");
}

#[test]
fn a_label_that_already_has_spaces_does_not_double_them() {
    // A `popup_title_format` written with its own padding is trimmed first, so
    // the border never shows two spaces on one side.
    assert_eq!(popup_top_border(" rp "), "┌ rp ──────┐");
}

#[test]
fn a_title_too_long_for_the_border_keeps_both_spaces() {
    // 12 cells leave 8 for the label, so it truncates with an ellipsis instead
    // of spending the trailing space or the corner on more text.
    assert_eq!(popup_top_border("popup title"), "┌ popup t… ┐");
}

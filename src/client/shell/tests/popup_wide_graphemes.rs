use super::*;

use crate::protocol::render_ansi::symbol_cell_width;

/// A background pane filled with double-width graphemes on both column
/// parities, so wherever the centered popup lands one of the rows puts a wide
/// head immediately left of its border and another puts one immediately left
/// of the column just past it.
fn surface_with_popup_over_wide_background() -> PaneSurfaceFrame {
    let mut surface = surface_with_popup();
    let mut background = Buffer::empty(Rect::new(0, 0, 106, 20));
    for y in 0..background.area.height {
        let mut x = y % 2;
        while x + 1 < background.area.width {
            if let Some(cell) = background.cell_mut((x, y)) {
                cell.set_symbol("あ");
            }
            if let Some(cell) = background.cell_mut((x + 1, y)) {
                cell.set_symbol("");
            }
            x += 2;
        }
    }
    surface.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&background, None, &[]);
    surface
}

#[test]
fn popup_border_survives_wide_graphemes_in_the_background() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface_with_popup_over_wide_background());

    let frame = state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit geometry").rect;
    assert!(popup.x > 0, "the popup must not be flush with the frame");
    assert!(popup.right() < frame.width);

    let symbol = |x: u16, y: u16| {
        frame.cells[y as usize * frame.width as usize + x as usize]
            .symbol
            .as_str()
    };

    for y in popup.y..popup.bottom() {
        // A surviving wide head one column left of the popup is painted across
        // the border column, and makes the encoder skip it entirely.
        assert!(
            symbol_cell_width(symbol(popup.x - 1, y)) <= 1,
            "row {y}: a wide grapheme left of the popup covers its border column"
        );
        // A continuation cell whose head was overwritten by the border emits no
        // bytes, so the host keeps showing the stale right half of the glyph.
        assert_ne!(
            symbol(popup.right(), y),
            "",
            "row {y}: orphaned wide-grapheme half right of the popup"
        );
    }
}

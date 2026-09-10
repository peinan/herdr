use super::*;

use crate::protocol::render_ansi::{is_wide_continuation, is_wide_head};

/// A pane surface filled with double-width graphemes on both column parities,
/// so wherever a floating rect lands, some row cuts a grapheme in half at each
/// of its edges.
fn wide_background(cols: u16, rows: u16) -> FrameData {
    let mut background = Buffer::empty(Rect::new(0, 0, cols, rows));
    for y in 0..rows {
        let mut x = y % 2;
        while x + 1 < cols {
            if let Some(cell) = background.cell_mut((x, y)) {
                cell.set_symbol("あ");
            }
            if let Some(cell) = background.cell_mut((x + 1, y)) {
                cell.set_symbol("");
            }
            x += 2;
        }
    }
    FrameData::from_ratatui_buffer_with_hyperlinks(&background, None, &[])
}

fn surface_over_wide_background(popup: bool) -> PaneSurfaceFrame {
    let mut surface = if popup {
        surface_with_popup()
    } else {
        surface()
    };
    surface.frame = wide_background(106, 20);
    surface
}

/// Every wide grapheme in the composed frame must still own both of its
/// columns.
///
/// The encoder paints a head across the following cell and then skips it, and
/// it emits no bytes at all for a continuation, so a half pair always renders
/// one column from the wrong content: a stranded head eats its neighbour, and
/// a stranded continuation keeps whatever the host already showed. A head in
/// the frame's last column is fine — the terminal clips it.
fn assert_wide_graphemes_are_paired(frame: &FrameData) {
    for y in 0..frame.height {
        let mut owed_continuation = false;
        for x in 0..frame.width {
            let symbol = frame.cells[y as usize * frame.width as usize + x as usize]
                .symbol
                .as_str();
            if owed_continuation {
                assert!(
                    is_wide_continuation(symbol),
                    "wide grapheme at ({}, {y}) lost its second column",
                    x - 1
                );
            } else {
                assert!(
                    !is_wide_continuation(symbol),
                    "orphaned wide-grapheme half at ({x}, {y})"
                );
            }
            owed_continuation = is_wide_head(symbol);
        }
    }
}

fn compose_over_wide_background(popup: bool, prepare: impl FnOnce(&mut ClientShellState)) {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface_over_wide_background(popup));
    prepare(&mut state);

    let frame = state.compose(106, 20).expect("composed frame");

    assert_wide_graphemes_are_paired(&frame);
}

#[test]
fn the_popup_border_survives_wide_graphemes_in_the_background() {
    compose_over_wide_background(true, |state| {
        assert!(state.overlay.is_none());
    });
}

#[test]
fn modal_overlay_borders_survive_wide_graphemes_in_the_background() {
    // Every modal is framed by the same `panel()` helper, so onboarding stands
    // in for help, settings, rename, the worktree modals and the menus.
    compose_over_wide_background(false, |state| {
        state.overlay = Some(ClientShellOverlay::Onboarding);
    });
}

#[test]
fn a_clipped_blit_does_not_strand_a_wide_head_at_its_edge() {
    // Clipping to the target rect can stop copying between a head and its
    // continuation, leaving a glyph that would paint over the column beside
    // the surface.
    let source = wide_background(6, 1);
    let mut target = FrameData::from_ratatui_buffer_with_hyperlinks(
        &Buffer::empty(Rect::new(0, 0, 8, 1)),
        None,
        &[],
    );

    blit_pane_surface(&mut target, &source, Rect::new(0, 0, 5, 1));

    assert_wide_graphemes_are_paired(&target);
}

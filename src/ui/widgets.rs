use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Color,
};

use crate::app::state::Palette;
use crate::protocol::render_ansi::{is_wide_continuation, is_wide_head};

pub(super) fn panel_contrast_fg(palette: &Palette) -> Color {
    match palette.panel_bg {
        Color::Reset => palette.surface_dim,
        color => color,
    }
}

pub(crate) fn centered_popup_rect(area: Rect, popup_width: u16, popup_height: u16) -> Option<Rect> {
    let popup_width = popup_width.min(area.width.saturating_sub(4));
    let popup_height = popup_height.min(area.height.saturating_sub(2));
    if popup_width < 4 || popup_height < 4 {
        return None;
    }

    Some(Rect::new(
        area.x + area.width.saturating_sub(popup_width) / 2,
        area.y + area.height.saturating_sub(popup_height) / 2,
        popup_width,
        popup_height,
    ))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModalStackAreas {
    pub header: Rect,
    pub content: Rect,
    pub footer: Option<Rect>,
    pub actions: Option<Rect>,
}

pub(crate) fn modal_stack_areas(
    inner: Rect,
    header_height: u16,
    footer_height: u16,
    actions_height: u16,
    gap: u16,
) -> ModalStackAreas {
    #[derive(Clone, Copy)]
    enum Slot {
        Header,
        Content,
        Footer,
        Actions,
    }

    let mut constraints = Vec::new();
    let mut slots = Vec::new();
    let mut push = |slot: Slot, constraint: Constraint| {
        if !slots.is_empty() {
            constraints.push(Constraint::Length(gap));
        }
        constraints.push(constraint);
        slots.push(slot);
    };

    push(Slot::Header, Constraint::Length(header_height));
    push(Slot::Content, Constraint::Min(0));
    if footer_height > 0 {
        push(Slot::Footer, Constraint::Length(footer_height));
    }
    if actions_height > 0 {
        push(Slot::Actions, Constraint::Length(actions_height));
    }

    let areas = Layout::vertical(constraints).split(inner);
    let mut result = ModalStackAreas {
        header: Rect::default(),
        content: Rect::default(),
        footer: None,
        actions: None,
    };
    for (slot, area) in slots.into_iter().zip(areas.iter().step_by(2).copied()) {
        match slot {
            Slot::Header => result.header = area,
            Slot::Content => result.content = area,
            Slot::Footer => result.footer = Some(area),
            Slot::Actions => result.actions = Some(area),
        }
    }
    result
}

fn action_button_width(hint: Option<&str>, label: &str) -> u16 {
    match hint {
        Some(hint) => format!(" {hint} {label} ").chars().count() as u16,
        None => format!(" {label} ").chars().count() as u16,
    }
}

pub(crate) fn close_button_rect(area: Rect) -> Rect {
    let width = action_button_width(Some("esc"), "close");
    Rect::new(area.x + area.width.saturating_sub(width), area.y, width, 1)
}

pub(crate) fn continue_button_rect(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y,
        action_button_width(Some("↵"), "continue"),
        1,
    )
}

/// Blanks the halves of double-width graphemes left dangling across the
/// vertical edges of `area` after something was drawn into it.
///
/// A wide grapheme is stored as a head cell holding the glyph plus a
/// continuation cell holding an empty symbol, and the ANSI encoder derives
/// cursor advance purely from the current frame: it paints a head across the
/// following column and skips it, and it emits no bytes at all for a
/// continuation. A rect-bounded write only rewrites cells inside `area`, so
/// any pair straddling an edge is left half rewritten and one of the two
/// columns is then rendered from the wrong content. Three repairs, per row:
///
/// - a head one column left of `area`, whose second column `area` just took
/// - a head in `area`'s last column, whose second column is outside `area`
/// - a continuation one column right of `area`, whose head `area` just took
///
/// The last two only apply when `area` has a neighbour to the right at all.
/// Flush with the buffer's edge there is no other content to corrupt, and a
/// trailing wide glyph is the terminal's own clipping problem.
pub(crate) fn repair_wide_grapheme_edges(buffer: &mut Buffer, area: Rect) {
    if area.is_empty() {
        return;
    }
    let has_right_neighbour = area.right() < buffer.area.right();
    for y in area.top()..area.bottom() {
        if area.left() > 0 {
            blank_wide_head(buffer, area.left() - 1, y);
        }
        if has_right_neighbour {
            blank_wide_head(buffer, area.right() - 1, y);
            blank_orphan_continuation(buffer, area.right(), y);
        }
    }
}

fn blank_wide_head(buffer: &mut Buffer, x: u16, y: u16) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        if is_wide_head(cell.symbol()) {
            cell.set_symbol(" ");
        }
    }
}

fn blank_orphan_continuation(buffer: &mut Buffer, x: u16, y: u16) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        if is_wide_continuation(cell.symbol()) {
            cell.set_symbol(" ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes a double-width grapheme as the terminal renderer does: the glyph
    /// on the head cell and an empty symbol on the continuation cell.
    fn set_wide(buffer: &mut Buffer, x: u16, y: u16, symbol: &str) {
        buffer.cell_mut((x, y)).unwrap().set_symbol(symbol);
        buffer.cell_mut((x + 1, y)).unwrap().set_symbol("");
    }

    fn symbol_at(buffer: &Buffer, x: u16, y: u16) -> &str {
        buffer.cell((x, y)).unwrap().symbol()
    }

    #[test]
    fn blanks_a_wide_head_whose_second_column_the_rect_took() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 2));
        set_wide(&mut buffer, 3, 0, "あ");
        // Row 1 keeps a narrow neighbour to prove untouched columns survive.
        buffer.cell_mut((3, 1)).unwrap().set_symbol("x");
        let rect = Rect::new(4, 0, 4, 2);
        buffer.set_string(4, 0, "····", ratatui::style::Style::default());
        buffer.set_string(4, 1, "····", ratatui::style::Style::default());

        repair_wide_grapheme_edges(&mut buffer, rect);

        assert_eq!(symbol_at(&buffer, 3, 0), " ");
        assert_eq!(symbol_at(&buffer, 3, 1), "x");
    }

    #[test]
    fn blanks_a_continuation_whose_head_the_rect_took() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        // The head at column 6 is inside the rect and has been overwritten by
        // it; only the continuation at column 7 is left behind.
        set_wide(&mut buffer, 6, 0, "い");
        let rect = Rect::new(3, 0, 4, 1);
        buffer.set_string(3, 0, "····", ratatui::style::Style::default());

        repair_wide_grapheme_edges(&mut buffer, rect);

        assert_eq!(symbol_at(&buffer, 7, 0), " ");
    }

    #[test]
    fn blanks_a_wide_head_the_rect_left_in_its_last_column() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        // A clipped copy can end on a head whose second column was never
        // written, so the glyph would paint over content the rect does not own.
        set_wide(&mut buffer, 6, 0, "う");

        repair_wide_grapheme_edges(&mut buffer, Rect::new(3, 0, 4, 1));

        assert_eq!(symbol_at(&buffer, 6, 0), " ");
        assert_eq!(symbol_at(&buffer, 7, 0), " ");
    }

    #[test]
    fn leaves_a_trailing_wide_head_alone_at_the_buffer_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        set_wide(&mut buffer, 4, 0, "え");

        repair_wide_grapheme_edges(&mut buffer, Rect::new(0, 0, 6, 1));

        // There is no neighbouring content to corrupt, and the terminal clips
        // a trailing glyph itself.
        assert_eq!(symbol_at(&buffer, 4, 0), "え");
        assert_eq!(symbol_at(&buffer, 5, 0), "");
    }

    #[test]
    fn empty_rects_are_a_no_op() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        set_wide(&mut buffer, 1, 0, "お");

        repair_wide_grapheme_edges(&mut buffer, Rect::new(3, 0, 0, 1));

        assert_eq!(symbol_at(&buffer, 1, 0), "お");
    }
}

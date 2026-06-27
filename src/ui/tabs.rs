use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
    Frame,
};

use crate::app::AppState;

const NEW_TAB_WIDTH: u16 = 3;
/// Width (in cells) of a single tab marker slot: the glyph plus one trailing
/// space. Also the click target width for switching to that tab.
const TAB_MARKER_WIDTH: u16 = 2;
/// Blank cells of padding before the first tab marker.
const TAB_BAR_LEFT_PAD: u16 = 1;

/// Active tab marker. Nerd Font glyph (Material Design Icons range); requires a
/// Nerd Font to render (otherwise the terminal shows a missing-glyph box).
const TAB_MARKER_ACTIVE: char = '\u{F09DE}';
/// Inactive tab marker. Nerd Font glyph (Codicon range); requires a Nerd Font.
const TAB_MARKER_INACTIVE: char = '\u{EABC}';

#[derive(Debug, Clone, Default)]
pub(crate) struct TabBarView {
    pub scroll: usize,
    pub tab_hit_areas: Vec<Rect>,
    pub new_tab_hit_area: Rect,
}

/// The active tab's name, shown right-aligned in the bar — but only when the
/// tab has a user-assigned custom name (auto-named tabs show nothing here).
fn active_tab_name(ws: &crate::workspace::Workspace) -> Option<String> {
    ws.tabs
        .get(ws.active_tab)
        .filter(|tab| !tab.is_auto_named())
        .and_then(|_| ws.active_tab_display_name())
}

/// Cells reserved on the right for the active tab name: its display width plus
/// one cell of padding between the markers/`+` and the name.
fn name_region_width(name: &str) -> u16 {
    Line::from(name).width() as u16 + 1
}

fn layout_tab_hit_areas(ws: &crate::workspace::Workspace, area: Rect, scroll: usize) -> Vec<Rect> {
    let mut rects = vec![Rect::default(); ws.tabs.len()];
    if area.width == 0 || area.height == 0 {
        return rects;
    }

    let mut x = area.x;
    let right = area.x + area.width;
    for rect in rects.iter_mut().skip(scroll) {
        if x >= right {
            break;
        }
        let width = TAB_MARKER_WIDTH.min(right.saturating_sub(x)).max(1);
        *rect = Rect::new(x, area.y, width, 1);
        x = x.saturating_add(TAB_MARKER_WIDTH);
    }
    rects
}

fn centered_tab_scroll(ws: &crate::workspace::Workspace, area: Rect) -> usize {
    let mut best_scroll = ws.active_tab;
    let mut best_distance = u16::MAX;
    let viewport_center = area.x.saturating_mul(2).saturating_add(area.width);

    for scroll in 0..=ws.active_tab {
        let rects = layout_tab_hit_areas(ws, area, scroll);
        let Some(active_rect) = rects.get(ws.active_tab).copied() else {
            continue;
        };
        if active_rect.width == 0 {
            continue;
        }

        let active_center = active_rect
            .x
            .saturating_mul(2)
            .saturating_add(active_rect.width);
        let distance = active_center.abs_diff(viewport_center);
        if distance <= best_distance {
            best_distance = distance;
            best_scroll = scroll;
        }
    }

    best_scroll
}

fn trailing_tab_controls_x(tab_hit_areas: &[Rect], fallback_x: u16) -> u16 {
    tab_hit_areas
        .iter()
        .rev()
        .find(|rect| rect.width > 0)
        .map(|rect| rect.x + rect.width)
        .unwrap_or(fallback_x)
}

fn max_tab_scroll(ws: &crate::workspace::Workspace, area: Rect) -> usize {
    (0..ws.tabs.len())
        .find(|&scroll| {
            layout_tab_hit_areas(ws, area, scroll)
                .last()
                .is_some_and(|rect| rect.width > 0)
        })
        .unwrap_or(0)
}

pub(crate) fn compute_tab_bar_view(
    ws: &crate::workspace::Workspace,
    area: Rect,
    current_scroll: usize,
    follow_active: bool,
    mouse_chrome: bool,
) -> TabBarView {
    if area.width == 0 || area.height == 0 {
        return TabBarView::default();
    }

    // Reserve space on the right for the active tab name (when it has one).
    let name_w = active_tab_name(ws)
        .map(|name| name_region_width(&name))
        .unwrap_or(0)
        .min(area.width);
    let controls_w = area.width.saturating_sub(name_w);
    // One blank cell of padding before the first marker.
    let left_pad = TAB_BAR_LEFT_PAD.min(controls_w);
    let inner_w = controls_w.saturating_sub(left_pad);

    // Reserve trailing space within the controls region for the "+" button.
    let plus_w = if mouse_chrome {
        NEW_TAB_WIDTH.min(inner_w)
    } else {
        0
    };
    let markers_area = Rect::new(
        area.x + left_pad,
        area.y,
        inner_w.saturating_sub(plus_w),
        area.height,
    );

    let max_scroll = max_tab_scroll(ws, markers_area);
    let scroll = if follow_active {
        centered_tab_scroll(ws, markers_area).min(max_scroll)
    } else {
        current_scroll.min(max_scroll)
    };
    let tab_hit_areas = layout_tab_hit_areas(ws, markers_area, scroll);

    let new_tab_hit_area = if plus_w > 0 {
        let plus_x = trailing_tab_controls_x(&tab_hit_areas, markers_area.x)
            .min(markers_area.x + markers_area.width);
        Rect::new(plus_x, area.y, plus_w, 1)
    } else {
        Rect::default()
    };

    TabBarView {
        scroll,
        tab_hit_areas,
        new_tab_hit_area,
    }
}

/// Column for the drag-to-reorder drop indicator, derived purely from the
/// visible marker rects (no scroll buttons exist).
fn tab_drop_indicator_x(app: &AppState, insert_idx: usize) -> Option<u16> {
    let visible: Vec<(usize, Rect)> = app
        .view
        .tab_hit_areas
        .iter()
        .enumerate()
        .filter(|(_, rect)| rect.width > 0)
        .map(|(idx, rect)| (idx, *rect))
        .collect();
    let (first_idx, first_rect) = *visible.first()?;
    let (_, last_rect) = *visible.last()?;

    if insert_idx <= first_idx {
        return Some(first_rect.x);
    }
    if let Some((_, rect)) = visible.iter().find(|(idx, _)| *idx == insert_idx) {
        return Some(rect.x.saturating_sub(1));
    }
    Some(last_rect.x + last_rect.width)
}

pub(super) fn render_tab_bar(app: &AppState, frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let Some(active_ws_idx) = app.active else {
        return;
    };
    let Some(ws) = app.workspaces.get(active_ws_idx) else {
        return;
    };

    let p = &app.palette;

    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(Style::default().bg(p.panel_bg)),
        area,
    );

    // Tab markers: one glyph per tab, distinguished by colour (active accent,
    // inactive grey). The active marker turns yellow in prefix-highlight mode.
    for (idx, rect) in app.view.tab_hit_areas.iter().enumerate() {
        if rect.width == 0 {
            continue;
        }
        let (glyph, fg) = if idx == ws.active_tab {
            let fg = if app.prefix_highlight_active() {
                p.yellow
            } else {
                p.accent
            };
            (TAB_MARKER_ACTIVE, fg)
        } else {
            (TAB_MARKER_INACTIVE, p.overlay1)
        };
        let mut buf = [0u8; 4];
        frame.buffer_mut()[(rect.x, rect.y)]
            .set_symbol(glyph.encode_utf8(&mut buf))
            .set_style(Style::default().fg(fg));
    }

    // Drag-to-reorder drop indicator.
    if let Some(crate::app::state::DragState {
        target:
            crate::app::state::DragTarget::TabReorder {
                ws_idx,
                insert_idx: Some(insert_idx),
                ..
            },
    }) = &app.drag
    {
        if *ws_idx == active_ws_idx {
            if let Some(x) = tab_drop_indicator_x(app, *insert_idx) {
                frame.buffer_mut()[(x.min(area.x + area.width.saturating_sub(1)), area.y)]
                    .set_symbol("│")
                    .set_style(Style::default().fg(p.accent));
            }
        }
    }

    // "+" new-tab button, placed right after the last marker.
    if app.mouse_capture && app.view.new_tab_hit_area.width > 0 {
        frame.render_widget(
            Paragraph::new(" + ").style(Style::default().fg(p.overlay1)),
            app.view.new_tab_hit_area,
        );
    }

    // Active tab name, right-aligned (only when the tab has a custom name).
    if let Some(name) = active_tab_name(ws) {
        let name_w = name_region_width(&name).min(area.width);
        let name_rect = Rect::new(
            area.x + area.width.saturating_sub(name_w),
            area.y,
            name_w,
            1,
        );
        frame.render_widget(
            Paragraph::new(name)
                .style(Style::default().fg(p.overlay1).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Right),
            name_rect,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::AppState;
    use crate::workspace::Workspace;
    use ratatui::{backend::TestBackend, Terminal};

    fn buffer_row_text(buffer: &ratatui::buffer::Buffer, area: Rect, row: u16) -> String {
        (area.x..area.x + area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn tab_bar_does_not_mark_zoomed_tabs() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.tabs[0].zoomed = true;
        let custom_tab = ws.test_add_tab(Some("test"));
        ws.tabs[custom_tab].zoomed = true;

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.view.tab_bar_rect = Rect::new(0, 0, 30, 1);
        let view = compute_tab_bar_view(&app.workspaces[0], app.view.tab_bar_rect, 0, true, false);
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_tab_bar(&app, frame, app.view.tab_bar_rect))
            .unwrap();

        let row = buffer_row_text(terminal.backend().buffer(), app.view.tab_bar_rect, 0);
        assert!(!row.contains('Z'), "zoom marker should not appear: {row:?}");
        assert_eq!(app.workspaces[0].tab_display_name(0).as_deref(), Some("1"));
        assert_eq!(
            app.workspaces[0].tab_display_name(custom_tab).as_deref(),
            Some("test")
        );
    }

    #[test]
    fn tab_bar_renders_active_and_inactive_markers() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.test_add_tab(Some("logs"));
        ws.switch_tab(0); // active = auto-named tab 0

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.view.tab_bar_rect = Rect::new(0, 0, 30, 1);
        let view = compute_tab_bar_view(&app.workspaces[0], app.view.tab_bar_rect, 0, true, false);
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_tab_bar(&app, frame, app.view.tab_bar_rect))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let active_rect = app.view.tab_hit_areas[0];
        let inactive_rect = app.view.tab_hit_areas[1];
        let active_cell = &buffer[(active_rect.x, active_rect.y)];
        let inactive_cell = &buffer[(inactive_rect.x, inactive_rect.y)];

        assert_eq!(active_cell.symbol().chars().next(), Some(TAB_MARKER_ACTIVE));
        assert_eq!(active_cell.style().fg, Some(app.palette.accent));
        assert_eq!(
            inactive_cell.symbol().chars().next(),
            Some(TAB_MARKER_INACTIVE)
        );
        assert_eq!(inactive_cell.style().fg, Some(app.palette.overlay1));
    }

    #[test]
    fn tab_bar_shows_active_custom_name_right_aligned() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        let custom = ws.test_add_tab(Some("logs"));
        ws.switch_tab(custom); // active = custom-named tab

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.mouse_capture = true;
        app.view.tab_bar_rect = Rect::new(0, 0, 30, 1);
        let view = compute_tab_bar_view(&app.workspaces[0], app.view.tab_bar_rect, 0, true, true);
        app.view.tab_hit_areas = view.tab_hit_areas;
        app.view.new_tab_hit_area = view.new_tab_hit_area;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_tab_bar(&app, frame, app.view.tab_bar_rect))
            .unwrap();
        let buffer = terminal.backend().buffer();

        let row = buffer_row_text(buffer, app.view.tab_bar_rect, 0);
        assert!(row.ends_with("logs"), "active name on the right: {row:?}");

        let name_x = app.view.tab_bar_rect.width - 4; // "logs" is 4 cells, flush right
        let name_style = buffer[(name_x, 0)].style();
        assert!(name_style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(name_style.fg, Some(app.palette.overlay1));
    }

    #[test]
    fn auto_named_active_tab_shows_no_name() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.test_add_tab(None); // both tabs auto-named
        ws.switch_tab(0);

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.view.tab_bar_rect = Rect::new(0, 0, 30, 1);
        let view = compute_tab_bar_view(&app.workspaces[0], app.view.tab_bar_rect, 0, true, false);
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_tab_bar(&app, frame, app.view.tab_bar_rect))
            .unwrap();

        let row = buffer_row_text(terminal.backend().buffer(), app.view.tab_bar_rect, 0);
        // Only the markers are present; no trailing name digits.
        assert!(
            !row.contains('1'),
            "auto-named tab should show no name: {row:?}"
        );
    }
}

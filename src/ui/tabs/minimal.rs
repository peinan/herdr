use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};

use super::TabBarView;
use crate::app::AppState;
use crate::config::TabBarAlign;

/// Width (in cells) of a single glyph-only marker slot: the glyph plus one
/// trailing space. Also the click target width for switching to that tab. When
/// the strip is right-aligned, the last slot's trailing space leaves a one-cell
/// gap before the right edge.
const TAB_MARKER_WIDTH: u16 = 2;

/// Active tab marker. Nerd Font glyph (Material Design Icons range); requires a
/// Nerd Font to render (otherwise the terminal shows a missing-glyph box).
const TAB_MARKER_ACTIVE: char = '\u{F09DE}';
/// Inactive tab marker. Nerd Font glyph (Codicon range); requires a Nerd Font.
const TAB_MARKER_INACTIVE: char = '\u{EABC}';

fn tab_chrome_label(ws: &crate::workspace::Workspace, tab_idx: usize) -> String {
    ws.tab_display_name(tab_idx)
        .unwrap_or_else(|| (tab_idx + 1).to_string())
}

/// Width of a single marker slot. Glyph-only slots are `TAB_MARKER_WIDTH`; when
/// titles are shown a slot is `glyph + space + name + trailing space`.
fn slot_width(ws: &crate::workspace::Workspace, tab_idx: usize, show_title: bool) -> u16 {
    if show_title {
        (tab_chrome_label(ws, tab_idx).chars().count() as u16).saturating_add(3)
    } else {
        TAB_MARKER_WIDTH
    }
}

fn total_width(ws: &crate::workspace::Workspace, show_title: bool) -> u16 {
    (0..ws.tabs.len())
        .map(|idx| slot_width(ws, idx, show_title))
        .fold(0u16, |acc, w| acc.saturating_add(w))
}

fn layout_tab_hit_areas(
    ws: &crate::workspace::Workspace,
    area: Rect,
    scroll: usize,
    show_title: bool,
) -> Vec<Rect> {
    let mut rects = vec![Rect::default(); ws.tabs.len()];
    if area.width == 0 || area.height == 0 {
        return rects;
    }

    let mut x = area.x;
    let right = area.x + area.width;
    for (idx, rect) in rects.iter_mut().enumerate().skip(scroll) {
        if x >= right {
            break;
        }
        let desired = slot_width(ws, idx, show_title);
        let width = desired.min(right.saturating_sub(x)).max(1);
        *rect = Rect::new(x, area.y, width, 1);
        x = x.saturating_add(desired);
    }
    rects
}

fn centered_tab_scroll(ws: &crate::workspace::Workspace, area: Rect, show_title: bool) -> usize {
    let mut best_scroll = ws.active_tab;
    let mut best_distance = u16::MAX;
    let viewport_center = area.x.saturating_mul(2).saturating_add(area.width);

    for scroll in 0..=ws.active_tab {
        let rects = layout_tab_hit_areas(ws, area, scroll, show_title);
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

fn max_tab_scroll(ws: &crate::workspace::Workspace, area: Rect, show_title: bool) -> usize {
    (0..ws.tabs.len())
        .find(|&scroll| {
            layout_tab_hit_areas(ws, area, scroll, show_title)
                .last()
                .is_some_and(|rect| rect.width > 0)
        })
        .unwrap_or(0)
}

pub(super) fn compute(
    ws: &crate::workspace::Workspace,
    area: Rect,
    current_scroll: usize,
    follow_active: bool,
    align: TabBarAlign,
    show_title: bool,
) -> TabBarView {
    let total_w = total_width(ws, show_title);
    let (markers_area, scroll) = if total_w <= area.width {
        // The strip fits: anchor it left or right. When right-aligned, the last
        // slot's trailing space leaves a one-cell gap before the right edge.
        let x = match align {
            TabBarAlign::Left => area.x,
            TabBarAlign::Right => area.x + area.width.saturating_sub(total_w),
        };
        (Rect::new(x, area.y, total_w, area.height), 0)
    } else {
        // Overflow: fill the bar from the left and keep the active marker visible.
        let markers_area = Rect::new(area.x, area.y, area.width, area.height);
        let max_scroll = max_tab_scroll(ws, markers_area, show_title);
        let scroll = if follow_active {
            centered_tab_scroll(ws, markers_area, show_title).min(max_scroll)
        } else {
            current_scroll.min(max_scroll)
        };
        (markers_area, scroll)
    };

    TabBarView {
        scroll,
        tab_hit_areas: layout_tab_hit_areas(ws, markers_area, scroll, show_title),
        // The scroll buttons and "+" button are not shown in this style.
        ..Default::default()
    }
}

/// Column for the drag-to-reorder drop indicator, derived purely from the
/// visible marker rects.
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

pub(super) fn render(app: &AppState, frame: &mut Frame, area: Rect) {
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
    // When `tab_bar_title` is set, the tab name is rendered after the glyph.
    let show_title = app.tab_bar_title;
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
        if show_title {
            let name = tab_chrome_label(ws, idx);
            frame.render_widget(
                Paragraph::new(format!("{glyph} {name}")).style(Style::default().fg(fg)),
                *rect,
            );
        } else {
            let mut buf = [0u8; 4];
            frame.buffer_mut()[(rect.x, rect.y)]
                .set_symbol(glyph.encode_utf8(&mut buf))
                .set_style(Style::default().fg(fg));
        }
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
        let view = compute(
            &app.workspaces[0],
            app.view.tab_bar_rect,
            0,
            true,
            TabBarAlign::Right,
            false,
        );
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(&app, frame, app.view.tab_bar_rect))
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
        let view = compute(
            &app.workspaces[0],
            app.view.tab_bar_rect,
            0,
            true,
            TabBarAlign::Right,
            false,
        );
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(&app, frame, app.view.tab_bar_rect))
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
    fn tab_markers_are_right_aligned_with_a_trailing_gap() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.test_add_tab(Some("logs")); // 2 tabs
        ws.switch_tab(0);

        app.workspaces = vec![ws];
        app.active = Some(0);
        let bar = Rect::new(0, 0, 30, 1);
        app.view.tab_bar_rect = bar;
        let view = compute(&app.workspaces[0], bar, 0, true, TabBarAlign::Right, false);
        app.view.tab_hit_areas = view.tab_hit_areas;

        // Last marker slot ends flush with the bar's right edge, so its trailing
        // space is the one-cell gap; the rightmost glyph sits at width - 2.
        let last = *app.view.tab_hit_areas.last().unwrap();
        assert_eq!(last.x + last.width, bar.x + bar.width);
        // No "+" button in this style.
        assert_eq!(view.new_tab_hit_area, Rect::default());
    }

    #[test]
    fn tab_markers_left_align_from_the_bar_start() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.test_add_tab(Some("logs")); // 2 tabs
        ws.switch_tab(0);

        app.workspaces = vec![ws];
        app.active = Some(0);
        let bar = Rect::new(0, 0, 30, 1);
        app.view.tab_bar_rect = bar;
        let view = compute(&app.workspaces[0], bar, 0, true, TabBarAlign::Left, false);

        // Left alignment anchors the first marker at the bar's left edge.
        let first = *view.tab_hit_areas.first().unwrap();
        assert_eq!(first.x, bar.x);
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
        let view = compute(
            &app.workspaces[0],
            app.view.tab_bar_rect,
            0,
            true,
            TabBarAlign::Right,
            false,
        );
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(&app, frame, app.view.tab_bar_rect))
            .unwrap();

        let row = buffer_row_text(terminal.backend().buffer(), app.view.tab_bar_rect, 0);
        assert!(
            !row.contains('1'),
            "tab title is hidden when tab_bar_title is off: {row:?}"
        );
    }

    #[test]
    fn tab_titles_render_names_when_enabled() {
        let mut app = AppState::test_new();
        let mut ws = Workspace::test_new("test");
        ws.tabs[0].set_custom_name("logs".into());
        ws.switch_tab(0);

        app.workspaces = vec![ws];
        app.active = Some(0);
        app.tab_bar_title = true;
        let bar = Rect::new(0, 0, 30, 1);
        app.view.tab_bar_rect = bar;
        let view = compute(&app.workspaces[0], bar, 0, true, TabBarAlign::Right, true);
        app.view.tab_hit_areas = view.tab_hit_areas;

        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(&app, frame, app.view.tab_bar_rect))
            .unwrap();

        let row = buffer_row_text(terminal.backend().buffer(), bar, 0);
        assert!(
            row.contains("logs"),
            "tab name should render alongside the glyph: {row:?}"
        );
    }
}

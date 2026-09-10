use super::*;

const TAB_SCROLL_BUTTON_WIDTH: u16 = 3;
const MIN_TAB_STRIP_WIDTH: u16 =
    MIN_TAB_WIDTH + NEW_TAB_WIDTH + TAB_SCROLL_BUTTON_WIDTH.saturating_mul(2);

/// Width of one glyph-only marker slot in the minimal style: the glyph plus one
/// trailing space, which is also the click target for that tab. Right-aligned
/// strips end on that space, leaving a one-cell gap before the bar's edge.
const TAB_MARKER_WIDTH: u16 = 2;
/// Active tab marker. Nerd Font glyph (Material Design Icons range); requires a
/// Nerd Font (otherwise the terminal shows a missing-glyph box).
const TAB_MARKER_ACTIVE: char = '\u{F09DE}';
/// Inactive tab marker. Nerd Font glyph (Codicon range); requires a Nerd Font.
const TAB_MARKER_INACTIVE: char = '\u{EABC}';

pub(crate) fn render_tab_bar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    tab_scroll: &mut usize,
    reveal_focused_tab: &mut bool,
    tab_drag_insert_index: Option<usize>,
    prefix_highlight: bool,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    buffer.set_style(area, Style::default().bg(palette.panel_bg));
    let tabs = snapshot
        .tabs
        .iter()
        .filter(|tab| Some(tab.workspace_id.as_str()) == snapshot.focused_workspace_id.as_deref())
        .collect::<Vec<_>>();
    match config.tab_bar_style {
        TabBarStyle::Classic => render_classic_tabs(
            buffer,
            area,
            snapshot,
            config,
            &tabs,
            tab_scroll,
            reveal_focused_tab,
            tab_drag_insert_index,
            prefix_highlight,
            hits,
        ),
        TabBarStyle::Minimal => render_minimal_tabs(
            buffer,
            area,
            snapshot,
            config,
            &tabs,
            tab_scroll,
            reveal_focused_tab,
            tab_drag_insert_index,
            hits,
        ),
    }
    render_tab_bar_status(buffer, area, snapshot, palette);
}

fn render_classic_tabs(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    tabs: &[&ClientShellTab],
    tab_scroll: &mut usize,
    reveal_focused_tab: &mut bool,
    tab_drag_insert_index: Option<usize>,
    prefix_highlight: bool,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    let desired_widths = tabs
        .iter()
        .map(|tab| {
            let label = tab_label(tab);
            display_width(&label).saturating_add(4).max(MIN_TAB_WIDTH)
        })
        .collect::<Vec<_>>();
    let content = tab_bar_content_area(snapshot, area);
    let mouse_chrome = config.mouse_capture;
    let new_tab_width = if mouse_chrome { NEW_TAB_WIDTH } else { 0 };
    let desired_total = desired_widths
        .iter()
        .copied()
        .fold(0_u16, u16::saturating_add)
        .saturating_add(tabs.len().saturating_sub(1).min(u16::MAX as usize) as u16)
        .saturating_add(new_tab_width);
    let overflow =
        desired_total > content.width && (!mouse_chrome || content.width >= MIN_TAB_STRIP_WIDTH);
    let available = if overflow && mouse_chrome {
        content
            .width
            .saturating_sub(NEW_TAB_WIDTH)
            .saturating_sub(TAB_SCROLL_BUTTON_WIDTH.saturating_mul(2))
    } else {
        content.width.saturating_sub(new_tab_width)
    };
    let max_scroll = max_tab_scroll(&desired_widths, available, 1);
    if !overflow {
        *tab_scroll = 0;
    } else if *reveal_focused_tab {
        if let Some(focused) = tabs.iter().position(|tab| tab.focused) {
            *tab_scroll =
                centered_tab_scroll(focused, &desired_widths, available, 1).min(max_scroll);
        }
    } else {
        *tab_scroll = (*tab_scroll).min(max_scroll);
    }
    *reveal_focused_tab = false;

    let mut x = content.x;
    let tab_right = if overflow && mouse_chrome {
        hits.tab_scroll_left = Rect::new(
            content.x,
            content.y,
            TAB_SCROLL_BUTTON_WIDTH.min(content.width),
            1,
        );
        put_text(
            buffer,
            hits.tab_scroll_left.x,
            content.y,
            hits.tab_scroll_left.width,
            " < ",
            Style::default()
                .fg(if *tab_scroll > 0 {
                    palette.overlay1
                } else {
                    palette.overlay0
                })
                .bg(palette.surface0),
        );
        x = hits.tab_scroll_left.right();
        content
            .right()
            .saturating_sub(NEW_TAB_WIDTH + TAB_SCROLL_BUTTON_WIDTH)
    } else {
        content.right().saturating_sub(new_tab_width)
    };

    let mut first_visible = None;
    let mut last_visible = None;
    for (index, tab) in tabs.iter().enumerate().skip(*tab_scroll) {
        let name = tab_label(tab);
        let desired = desired_widths[index];
        let remaining = tab_right.saturating_sub(x);
        let width = desired.min(remaining);
        if width == 0 {
            break;
        }
        let rect = Rect::new(x, area.y, width, 1);
        let style = if tab.focused {
            let base = Style::default()
                .fg(panel_contrast_fg(palette))
                .bg(if prefix_highlight {
                    palette.yellow
                } else {
                    palette.accent
                });
            if tab.custom_label {
                base.add_modifier(Modifier::BOLD)
            } else {
                base
            }
        } else if tab.custom_label {
            Style::default().fg(palette.overlay1).bg(palette.surface0)
        } else {
            Style::default()
                .fg(palette.overlay0)
                .bg(palette.surface0)
                .add_modifier(Modifier::DIM)
        };
        let padding = width.saturating_sub(display_width(&name));
        let left = padding / 2;
        let text = format!(
            "{empty:left$}{name}{empty:right_padding$}",
            empty = "",
            left = left as usize,
            right_padding = padding.saturating_sub(left) as usize,
        );
        put_text(buffer, rect.x, rect.y, rect.width, &text, style);
        hits.tabs.push((rect, tab.tab_id.clone()));
        first_visible.get_or_insert(index);
        last_visible = Some(index);
        x = x.saturating_add(width + 1);
        if width < desired {
            break;
        }
    }

    if overflow && mouse_chrome {
        hits.tab_scroll_right = Rect::new(tab_right, area.y, TAB_SCROLL_BUTTON_WIDTH, 1);
        let can_scroll_right = last_visible.is_some_and(|index| index + 1 < tabs.len());
        put_text(
            buffer,
            hits.tab_scroll_right.x,
            area.y,
            hits.tab_scroll_right.width,
            " > ",
            Style::default()
                .fg(if can_scroll_right {
                    palette.overlay1
                } else {
                    palette.overlay0
                })
                .bg(palette.surface0),
        );
        hits.new_tab = Rect::new(
            hits.tab_scroll_right.right(),
            area.y,
            content
                .right()
                .saturating_sub(hits.tab_scroll_right.right())
                .min(NEW_TAB_WIDTH),
            1,
        );
    } else if mouse_chrome {
        hits.new_tab = Rect::new(
            x.min(content.right()),
            area.y,
            content.right().saturating_sub(x).min(NEW_TAB_WIDTH),
            1,
        );
    }
    if mouse_chrome {
        put_text(
            buffer,
            hits.new_tab.x,
            area.y,
            hits.new_tab.width,
            " + ",
            Style::default().fg(palette.overlay1).bg(palette.panel_bg),
        );
    }

    if first_visible.is_some_and(|index| index > 0) {
        let ellipsis_x = if hits.tab_scroll_left.width > 0 {
            hits.tab_scroll_left.right()
        } else {
            content.x
        };
        put_text(
            buffer,
            ellipsis_x,
            area.y,
            u16::from(ellipsis_x < content.right()),
            "…",
            Style::default().fg(palette.overlay0),
        );
    }
    if last_visible.is_some_and(|index| index + 1 < tabs.len()) {
        let ellipsis_x = if hits.tab_scroll_right.width > 0 {
            hits.tab_scroll_right.x.saturating_sub(1)
        } else {
            content.right().saturating_sub(1)
        };
        put_text(
            buffer,
            ellipsis_x,
            area.y,
            u16::from(ellipsis_x >= content.x && ellipsis_x < content.right()),
            "…",
            Style::default().fg(palette.overlay0),
        );
    }

    if let Some(insert_index) = tab_drag_insert_index {
        if let Some(indicator_x) = tab_drop_indicator_x(hits, tabs, insert_index) {
            put_text(
                buffer,
                indicator_x.min(content.right().saturating_sub(1)),
                area.y,
                1,
                "│",
                Style::default().fg(palette.accent),
            );
        }
    }
}

/// Compact glyph strip: one colored marker per tab, optionally followed by the
/// tab's name. It draws no "+" button and no scroll buttons, so the whole
/// content area is available to markers and overflow scrolls instead.
fn render_minimal_tabs(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    tabs: &[&ClientShellTab],
    tab_scroll: &mut usize,
    reveal_focused_tab: &mut bool,
    tab_drag_insert_index: Option<usize>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    let show_title = config.tab_bar_title;
    let desired_widths = tabs
        .iter()
        .map(|tab| minimal_slot_width(tab, show_title))
        .collect::<Vec<_>>();
    let content = tab_bar_content_area(snapshot, area);
    let desired_total = desired_widths
        .iter()
        .copied()
        .fold(0_u16, u16::saturating_add);
    let overflow = desired_total > content.width;
    // Slots carry their own trailing space, so they sit flush against each
    // other and the strip anchors to whichever edge `tab_bar_align` names.
    let (strip_x, available) = if overflow {
        (content.x, content.width)
    } else {
        let x = match config.tab_bar_align {
            TabBarAlign::Left => content.x,
            TabBarAlign::Right => content.right().saturating_sub(desired_total),
        };
        (x, desired_total)
    };
    let max_scroll = max_tab_scroll(&desired_widths, available, 0);
    if !overflow {
        *tab_scroll = 0;
    } else if *reveal_focused_tab {
        if let Some(focused) = tabs.iter().position(|tab| tab.focused) {
            *tab_scroll =
                centered_tab_scroll(focused, &desired_widths, available, 0).min(max_scroll);
        }
    } else {
        *tab_scroll = (*tab_scroll).min(max_scroll);
    }
    *reveal_focused_tab = false;

    let mut x = strip_x;
    let tab_right = strip_x.saturating_add(available);
    for (index, tab) in tabs.iter().enumerate().skip(*tab_scroll) {
        let desired = desired_widths[index];
        let width = desired.min(tab_right.saturating_sub(x));
        if width == 0 {
            break;
        }
        let rect = Rect::new(x, area.y, width, 1);
        let (marker, fg) = minimal_marker(tab, palette);
        let text = if show_title {
            format!("{marker} {}", tab_label(tab))
        } else {
            marker.to_string()
        };
        put_text(
            buffer,
            rect.x,
            rect.y,
            rect.width,
            &text,
            Style::default().fg(fg).bg(palette.panel_bg),
        );
        hits.tabs.push((rect, tab.tab_id.clone()));
        x = x.saturating_add(width);
        if width < desired {
            break;
        }
    }

    if let Some(insert_index) = tab_drag_insert_index {
        if let Some(indicator_x) = tab_drop_indicator_x(hits, tabs, insert_index) {
            put_text(
                buffer,
                indicator_x.min(content.right().saturating_sub(1)),
                area.y,
                1,
                "│",
                Style::default().fg(palette.accent),
            );
        }
    }
}

/// Width of one minimal marker slot. Glyph-only slots are `TAB_MARKER_WIDTH`;
/// titled slots hold `glyph + space + label + trailing space`. The zoom marker
/// rides on the label, so glyph-only strips never widen for it.
fn minimal_slot_width(tab: &ClientShellTab, show_title: bool) -> u16 {
    if show_title {
        display_width(&tab_label(tab)).saturating_add(3)
    } else {
        TAB_MARKER_WIDTH
    }
}

/// Glyph and foreground color for one minimal marker. The only place the
/// active marker's color is decided.
fn minimal_marker(tab: &ClientShellTab, palette: &Palette) -> (char, ratatui::style::Color) {
    if tab.focused {
        (TAB_MARKER_ACTIVE, palette.accent)
    } else {
        (TAB_MARKER_INACTIVE, palette.overlay1)
    }
}

pub(crate) fn tab_bar_status_width(snapshot: &ClientShellSnapshot) -> u16 {
    let content = snapshot.tab_bar_right.iter().fold(0u16, |width, segment| {
        width.saturating_add(display_width(&segment.text))
    });
    let separators = snapshot.tab_bar_right.len().saturating_sub(1);
    content.saturating_add(
        display_width(&snapshot.tab_bar_right_separator)
            .saturating_mul(separators.min(u16::MAX as usize) as u16),
    )
}

fn tab_bar_status_area(snapshot: &ClientShellSnapshot, area: Rect) -> Option<Rect> {
    let width = tab_bar_status_width(snapshot);
    if width == 0 {
        return None;
    }
    let reserved = width.saturating_add(1);
    (area.width.saturating_sub(reserved) >= MIN_TAB_STRIP_WIDTH)
        .then(|| Rect::new(area.right().saturating_sub(width), area.y, width, 1))
}

fn tab_bar_content_area(snapshot: &ClientShellSnapshot, area: Rect) -> Rect {
    let reserved = tab_bar_status_area(snapshot, area)
        .map(|status| status.width.saturating_add(1))
        .unwrap_or(0);
    Rect {
        width: area.width.saturating_sub(reserved),
        ..area
    }
}

fn render_tab_bar_status(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    palette: &Palette,
) {
    let Some(status) = tab_bar_status_area(snapshot, area) else {
        return;
    };
    let separator_width = display_width(&snapshot.tab_bar_right_separator);
    let mut x = status.x;
    for (index, segment) in snapshot.tab_bar_right.iter().enumerate() {
        if index > 0 && separator_width > 0 {
            put_text(
                buffer,
                x,
                area.y,
                separator_width,
                &snapshot.tab_bar_right_separator,
                Style::default().fg(palette.overlay0).bg(palette.panel_bg),
            );
            x = x.saturating_add(separator_width);
        }
        let width = display_width(&segment.text);
        let style = if segment.accent {
            Style::default()
                .fg(panel_contrast_fg(palette))
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.overlay1).bg(palette.panel_bg)
        };
        put_text(buffer, x, area.y, width, &segment.text, style);
        x = x.saturating_add(width);
    }
}

fn tab_drop_indicator_x(
    hits: &ShellHitMap,
    tabs: &[&ClientShellTab],
    insert_index: usize,
) -> Option<u16> {
    let visible = hits
        .tabs
        .iter()
        .filter_map(|(rect, tab_id)| {
            tabs.iter()
                .position(|tab| tab.tab_id == *tab_id)
                .map(|index| (index, *rect))
        })
        .collect::<Vec<_>>();
    let (first_index, first_rect) = *visible.first()?;
    let (last_index, last_rect) = *visible.last()?;
    // Without scroll buttons (minimal style, or mouse chrome off) the hidden
    // tabs have no on-screen handle, so the indicator clamps to the strip.
    if insert_index == 0 {
        return Some(if first_index == 0 || hits.tab_scroll_left.width == 0 {
            first_rect.x
        } else {
            hits.tab_scroll_left.right()
        });
    }
    if let Some((_, rect)) = visible.iter().find(|(index, _)| *index == insert_index) {
        return Some(rect.x.saturating_sub(1));
    }
    if insert_index >= tabs.len() {
        return Some(
            if last_index + 1 >= tabs.len() || hits.tab_scroll_right.width == 0 {
                last_rect.right()
            } else {
                hits.tab_scroll_right.x.saturating_sub(1)
            },
        );
    }
    None
}

/// `gap` is the number of blank cells the style leaves between slots: 1 for
/// the classic strip, 0 for the minimal one, whose slots include their own
/// trailing space.
fn centered_tab_scroll(focused: usize, widths: &[u16], available: u16, gap: u16) -> usize {
    let mut best = focused;
    let mut best_distance = u16::MAX;
    for start in 0..=focused {
        let before = widths
            .iter()
            .copied()
            .enumerate()
            .skip(start)
            .take(focused.saturating_sub(start))
            .fold(0u16, |width, (_, tab)| {
                width.saturating_add(tab.saturating_add(gap))
            });
        if before >= available {
            continue;
        }
        let focused_width = widths[focused].min(available.saturating_sub(before));
        let center = before.saturating_mul(2).saturating_add(focused_width);
        let distance = center.abs_diff(available);
        if distance <= best_distance {
            best_distance = distance;
            best = start;
        }
    }
    best
}

fn max_tab_scroll(widths: &[u16], available: u16, gap: u16) -> usize {
    (0..widths.len())
        .find(|start| {
            last_visible_tab(*start, widths, available, gap) == widths.len().checked_sub(1)
        })
        .unwrap_or(0)
}

fn last_visible_tab(start: usize, widths: &[u16], available: u16, gap: u16) -> Option<usize> {
    let mut remaining = available;
    let mut last = None;
    for (index, width) in widths.iter().copied().enumerate().skip(start) {
        if remaining == 0 {
            break;
        }
        last = Some(index);
        if width >= remaining {
            break;
        }
        remaining = remaining.saturating_sub(width.saturating_add(gap));
    }
    last
}

fn tab_label(tab: &ClientShellTab) -> String {
    if tab.zoomed {
        format!("{} Z", tab.label)
    } else {
        tab.label.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(label: &str, zoomed: bool) -> ClientShellTab {
        ClientShellTab {
            tab_id: "tab_1".into(),
            workspace_id: "ws_1".into(),
            number: 1,
            label: label.into(),
            custom_label: false,
            zoomed,
            focused: false,
            agent_status: crate::api::schema::AgentStatus::Idle,
        }
    }

    #[test]
    fn minimal_markers_occupy_a_single_cell() {
        assert_eq!(display_width(&TAB_MARKER_ACTIVE.to_string()), 1);
        assert_eq!(display_width(&TAB_MARKER_INACTIVE.to_string()), 1);
    }

    #[test]
    fn minimal_slot_width_covers_the_glyph_and_optional_label() {
        assert_eq!(minimal_slot_width(&tab("logs", false), false), 2);
        assert_eq!(minimal_slot_width(&tab("logs", true), false), 2);
        assert_eq!(minimal_slot_width(&tab("logs", false), true), 7);
        assert_eq!(minimal_slot_width(&tab("logs", true), true), 9);
    }

    #[test]
    fn tab_scroll_helpers_follow_the_style_gap() {
        assert_eq!(last_visible_tab(0, &[8, 8, 8], 17, 1), Some(1));
        assert_eq!(last_visible_tab(0, &[8, 8, 8], 17, 0), Some(2));
        assert_eq!(max_tab_scroll(&[2; 5], 6, 0), 2);
        assert_eq!(max_tab_scroll(&[2; 5], 6, 1), 3);
        assert_eq!(centered_tab_scroll(4, &[2; 5], 6, 0), 3);
    }
}

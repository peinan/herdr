mod classic;
mod minimal;

pub(crate) use classic::tab_bar_content_area;

use ratatui::{layout::Rect, Frame};

use crate::app::AppState;
use crate::config::{TabBarAlign, TabBarStyle};

/// Tab bar geometry: hit areas and scroll state derived from a layout pass so
/// that `render` and mouse hit-testing agree. Shared by both styles; the
/// classic style additionally populates the scroll-button and "+" hit areas,
/// which the minimal style leaves unset (zero-width).
#[derive(Debug, Clone, Default)]
pub(crate) struct TabBarView {
    pub scroll: usize,
    pub tab_hit_areas: Vec<Rect>,
    pub scroll_left_hit_area: Rect,
    pub scroll_right_hit_area: Rect,
    pub new_tab_hit_area: Rect,
}

/// Lay out the tab bar for the active style. Presentation-only.
///
/// `zoom_marker` is the string appended to zoomed tabs' labels (from
/// `AppState::tab_zoom_marker`), or `None` when the tab position is disabled.
/// It is folded into per-tab widths so the appended marker never overflows.
#[allow(clippy::too_many_arguments)] // Style/align/title/zoom parameterize one layout pass.
pub(crate) fn compute_tab_bar_view(
    ws: &crate::workspace::Workspace,
    area: Rect,
    current_scroll: usize,
    follow_active: bool,
    mouse_chrome: bool,
    style: TabBarStyle,
    align: TabBarAlign,
    title: bool,
    zoom_marker: Option<&str>,
) -> TabBarView {
    if area.width == 0 || area.height == 0 {
        return TabBarView::default();
    }
    match style {
        TabBarStyle::Classic => classic::compute(
            ws,
            area,
            current_scroll,
            follow_active,
            mouse_chrome,
            zoom_marker,
        ),
        TabBarStyle::Minimal => minimal::compute(
            ws,
            area,
            current_scroll,
            follow_active,
            align,
            title,
            zoom_marker,
        ),
    }
}

pub(super) fn render_tab_bar(app: &AppState, frame: &mut Frame, area: Rect) {
    match app.tab_bar_style {
        TabBarStyle::Classic => classic::render(app, frame, area),
        TabBarStyle::Minimal => minimal::render(app, frame, area),
    }
}

use super::*;

use crate::config::PopupBackground;
use crate::protocol::color_to_u32;

struct ComposedPopup {
    frame: FrameData,
    outer: Rect,
    inner: Rect,
}

impl ComposedPopup {
    fn background_at(&self, x: u16, y: u16) -> u32 {
        self.frame.cells[y as usize * self.frame.width as usize + x as usize].bg
    }

    /// A padding cell, between the border and the content.
    fn padding_background(&self) -> u32 {
        self.background_at(self.inner.x - 1, self.inner.y)
    }

    fn border_background(&self) -> u32 {
        self.background_at(self.outer.x, self.outer.y)
    }
}

/// A popup with padding, so the border ring and the padding ring are both
/// visible around the content the popup's own program draws.
fn compose_popup(background: &str, surface: PaneSurfaceFrame) -> ComposedPopup {
    let config: Config = toml::from_str(&format!(
        r#"
[ui]
popup_padding = {{ top = 1, left = 2 }}
popup_background = "{background}"
"#
    ))
    .expect("config");
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface);
    let frame = state.compose(106, 20).expect("popup frame");
    let hit = state.hits.popup.as_ref().expect("popup hit geometry");
    ComposedPopup {
        frame,
        outer: hit.rect,
        inner: hit.inner_rect,
    }
}

/// A popup whose program painted its first cell red and left the rest of the
/// content on the terminal's default background.
fn surface_with_colored_popup_cell() -> PaneSurfaceFrame {
    let mut surface = surface_with_popup();
    let mut popup_buffer = Buffer::with_lines(["popup-live", "", ""]);
    popup_buffer
        .cell_mut((0, 0))
        .expect("popup cell")
        .set_bg(ratatui::style::Color::Red);
    if let Some(popup) = surface.popup.as_deref_mut() {
        popup.frame = FrameData::from_ratatui_buffer_with_hyperlinks(&popup_buffer, None, &[]);
    }
    surface
}

fn panel_bg() -> u32 {
    color_to_u32(
        ClientShellConfig::from_config(&Config::default())
            .palette
            .panel_bg,
    )
}

fn terminal_default() -> u32 {
    color_to_u32(ratatui::style::Color::Reset)
}

#[test]
fn the_default_paints_only_the_popup_chrome() {
    let composed = compose_popup("panel", surface_with_popup());

    assert_eq!(composed.border_background(), panel_bg());
    assert_eq!(composed.padding_background(), panel_bg());
    // The content keeps whatever the program drew, here the terminal default.
    assert_eq!(
        composed.background_at(composed.inner.x, composed.inner.y),
        terminal_default()
    );
}

#[test]
fn transparent_leaves_the_whole_popup_on_the_terminal_background() {
    let composed = compose_popup("transparent", surface_with_popup());

    assert_eq!(composed.border_background(), terminal_default());
    assert_eq!(
        composed.background_at(composed.outer.right() - 1, composed.outer.bottom() - 1),
        terminal_default()
    );
    assert_eq!(composed.padding_background(), terminal_default());
    assert_eq!(
        composed.background_at(composed.inner.x, composed.inner.y),
        terminal_default()
    );
}

#[test]
fn all_fills_behind_the_content_too() {
    let composed = compose_popup("all", surface_with_popup());

    assert_eq!(composed.border_background(), panel_bg());
    assert_eq!(composed.padding_background(), panel_bg());
    assert_eq!(
        composed.background_at(composed.inner.x, composed.inner.y),
        panel_bg()
    );
    assert_eq!(
        composed.background_at(composed.inner.right() - 1, composed.inner.bottom() - 1),
        panel_bg()
    );
}

#[test]
fn all_keeps_a_background_the_popups_program_chose() {
    let composed = compose_popup("all", surface_with_colored_popup_cell());

    assert_eq!(
        composed.background_at(composed.inner.x, composed.inner.y),
        color_to_u32(ratatui::style::Color::Red)
    );
    // Its neighbour was on the terminal default, so it is filled.
    assert_eq!(
        composed.background_at(composed.inner.x + 1, composed.inner.y),
        panel_bg()
    );
}

#[test]
fn nothing_outside_the_popup_is_filled() {
    let composed = compose_popup("all", surface_with_popup());

    assert_eq!(
        composed.background_at(composed.outer.x - 1, composed.outer.y),
        terminal_default()
    );
}

#[test]
fn the_config_value_reaches_the_client_shell() {
    for (value, expected) in [
        ("panel", PopupBackground::Panel),
        ("all", PopupBackground::All),
        ("transparent", PopupBackground::Transparent),
    ] {
        let config: Config =
            toml::from_str(&format!("[ui]\npopup_background = \"{value}\"\n")).expect("config");
        assert_eq!(
            ClientShellConfig::from_config(&config).popup_background,
            expected
        );
    }
    assert_eq!(
        ClientShellConfig::from_config(&Config::default()).popup_background,
        PopupBackground::Panel
    );
}

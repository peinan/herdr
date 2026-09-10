use super::*;

use crate::config::PopupDimBackground;

fn client_config(dim: &str) -> ClientShellConfig {
    let config: Config =
        toml::from_str(&format!("[ui]\npopup_dim_background = \"{dim}\"\n")).expect("config");
    ClientShellConfig::from_config(&config)
}

struct ComposedPopup {
    frame: FrameData,
    popup: Rect,
    sidebar: Rect,
    pane_surface: Rect,
}

impl ComposedPopup {
    fn is_dim(&self, x: u16, y: u16) -> bool {
        let cell = &self.frame.cells[y as usize * self.frame.width as usize + x as usize];
        cell.modifier & ratatui::style::Modifier::DIM.bits() != 0
    }
}

fn compose_with_popup(config: ClientShellConfig) -> ComposedPopup {
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface_with_popup());
    let layout = state.layout(106, 20);
    let frame = state.compose(106, 20).expect("popup frame");
    let popup = state.hits.popup.as_ref().expect("popup hit geometry").rect;
    assert!(layout.sidebar.width > 0, "desktop layout expected");
    ComposedPopup {
        frame,
        popup,
        sidebar: layout.sidebar,
        pane_surface: layout.pane_surface,
    }
}

/// A pane-surface cell that the popup does not cover.
fn uncovered_pane_cell(composed: &ComposedPopup) -> (u16, u16) {
    let x = composed.pane_surface.x;
    let y = composed.pane_surface.y;
    assert!(
        x < composed.popup.x,
        "popup must not start at the pane edge"
    );
    (x, y)
}

#[test]
fn the_default_leaves_the_background_undimmed() {
    let composed = compose_with_popup(ClientShellConfig::from_config(&Config::default()));

    let (x, y) = uncovered_pane_cell(&composed);
    assert!(!composed.is_dim(x, y));
    assert!(!composed.is_dim(composed.sidebar.x, composed.sidebar.y));
}

#[test]
fn all_dims_the_sidebar_and_the_pane_area_but_not_the_popup() {
    let composed = compose_with_popup(client_config("all"));

    let (x, y) = uncovered_pane_cell(&composed);
    assert!(composed.is_dim(x, y));
    assert!(composed.is_dim(composed.sidebar.x, composed.sidebar.y));
    assert!(!composed.is_dim(composed.popup.x, composed.popup.y));
    assert!(!composed.is_dim(composed.popup.x + 1, composed.popup.y + 1));
}

#[test]
fn panes_dims_only_the_pane_area() {
    let composed = compose_with_popup(client_config("panes"));

    let (x, y) = uncovered_pane_cell(&composed);
    assert!(composed.is_dim(x, y));
    assert!(!composed.is_dim(composed.sidebar.x, composed.sidebar.y));
    assert!(!composed.is_dim(composed.popup.x, composed.popup.y));
}

#[test]
fn dimming_stops_when_the_popup_closes() {
    let mut state = ClientShellState::new(client_config("all"));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let layout = state.layout(106, 20);
    let frame = state.compose(106, 20).expect("frame");

    assert!(state.hits.popup.is_none());
    let cell = &frame.cells
        [layout.pane_surface.y as usize * frame.width as usize + layout.pane_surface.x as usize];
    assert_eq!(cell.modifier & ratatui::style::Modifier::DIM.bits(), 0);
}

#[test]
fn the_config_value_reaches_the_client_shell() {
    assert_eq!(
        client_config("panes").popup_dim_background,
        PopupDimBackground::Panes
    );
    assert_eq!(
        ClientShellConfig::from_config(&Config::default()).popup_dim_background,
        PopupDimBackground::Off
    );
}

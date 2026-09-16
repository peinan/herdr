//! Stable endpoint compatibility contract for client-owned shells.
//!
//! The endpoint generation is intentionally independent from the private
//! binary protocol used by same-install CLI, direct-terminal, and handoff
//! paths. Generation 1 is the compatibility floor for Local, SSH, and Cloud
//! shell endpoints and must remain available indefinitely unless retired for a
//! security reason. New JSON fields must be optional or have serde defaults;
//! new enum values need an `Unknown` fallback. Unknown named controls are
//! optional and ignored unless negotiated as part of the core.

use serde::{Deserialize, Serialize};

use super::{ClientShellSnapshot, ClientSurfaceSize, ServerMessage};

pub const ENDPOINT_PROTOCOL_GENERATION: u32 = 1;
pub const ENDPOINT_HELLO_KIND: &str = "endpoint.hello.v1";
pub const ENDPOINT_WELCOME_KIND: &str = "endpoint.welcome.v1";
pub const SNAPSHOT_CODEC_V1: &str = "shell.snapshot.v1";
pub const ENDPOINT_SNAPSHOT_KIND: &str = SNAPSHOT_CODEC_V1;
pub const SURFACE_CODEC_V1: &str = "shell.surface.v1";
/// The generation-1 surface stream plus one named control message.
///
/// This codec does not change a single surface byte: `ServerMessage::PaneSurface`
/// encodes exactly as it does under `shell.surface.v1`, and a v1 client is never
/// sent the extra control. The only difference is that a v2 client also receives
/// [`POPUP_SURFACE_METRICS_KIND`] immediately before every popup-bearing surface
/// frame. Anyone cutting a v3 should read that as the rule: the surface frames
/// are frozen at generation 1 and new surface facts ride the control lane.
pub const SURFACE_CODEC_V2: &str = "shell.surface.v2";
pub const POPUP_SURFACE_METRICS_KIND: &str = "shell.surface.popup.v2";
pub const INPUT_CODEC_V1: &str = "shell.input.semantic.v1";
pub const BLOB_CODEC_V1: &str = "shell.blob.v1";
pub const SURFACE_INTEREST_CAPABILITY: &str = "surface_interest";
pub const PRESENTATION_EFFECTS_FENCE_CAPABILITY: &str = "presentation_effects_fence";
pub const PRESENTATION_EFFECTS_SYNC_KIND: &str = "endpoint.presentation.sync.v1";
pub const PRESENTATION_EFFECTS_READY_KIND: &str = "endpoint.presentation.ready.v1";
pub const HEALTH_CHECK_CAPABILITY: &str = "health_check";
pub const HEALTH_PING_KIND: &str = "endpoint.health.ping.v1";
pub const HEALTH_PONG_KIND: &str = "endpoint.health.pong.v1";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointClientHello {
    pub generation: u32,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub surface_size: ClientSurfaceSize,
    pub pixel_mouse: bool,
    pub direct_graphics: bool,
    pub endpoint_keybindings: bool,
    pub mouse_capture: bool,
    #[serde(default = "default_true")]
    pub surface_active: bool,
    #[serde(default)]
    pub snapshot_codecs: Vec<String>,
    #[serde(default)]
    pub surface_codecs: Vec<String>,
    #[serde(default)]
    pub input_codecs: Vec<String>,
    #[serde(default)]
    pub blob_codecs: Vec<String>,
}

/// Popup terminal facts that the pane surface frame cannot carry.
///
/// The popup is not one of the tab's panes and `ClientShellPopupSurface` is
/// frozen at generation 1, so these ride the named control lane instead.
///
/// They are *not* safe to apply on arrival. A client maps a viewport row onto an
/// absolute scrollback row with `max_offset_from_bottom - offset_from_bottom`, so
/// metrics from a different frame than the one on screen silently resolve a
/// selection to the wrong rows — and a scroll-only move does not change
/// `content_revision`, so no staleness guard would catch it. `surface_revision`
/// names the one surface frame these metrics belong to; a client must stage them
/// and commit only against that frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopupSurfaceMetrics {
    /// Revision of the popup surface frame these metrics describe.
    pub surface_revision: u64,
    pub terminal_id: String,
    pub content_revision: u64,
    /// Whether the popup terminal is on its alternate screen. A switch replaces
    /// the whole buffer without necessarily changing the popup's size, so a
    /// selection anchored to the old screen has to be dropped.
    #[serde(default)]
    pub alternate_screen_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<crate::protocol::PaneSurfaceScrollMetrics>,
}

impl PopupSurfaceMetrics {
    /// Whether both describe the same popup state, ignoring the frame stamp.
    ///
    /// The server compares metrics before a surface revision is assigned, so the
    /// stamp cannot take part in change detection.
    pub fn describes_same_state(&self, other: &Self) -> bool {
        self.terminal_id == other.terminal_id
            && self.content_revision == other.content_revision
            && self.alternate_screen_active == other.alternate_screen_active
            && self.scroll == other.scroll
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointHandshakeError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointServerWelcome {
    pub generation: u32,
    pub server_version: String,
    pub snapshot_codec: String,
    pub surface_codec: String,
    pub input_codec: String,
    pub blob_codec: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<EndpointHandshakeError>,
}

pub fn snapshot_message(snapshot: &ClientShellSnapshot) -> serde_json::Result<ServerMessage> {
    Ok(ServerMessage::EndpointControl {
        kind: ENDPOINT_SNAPSHOT_KIND.into(),
        data: serde_json::to_string(snapshot)?,
    })
}

impl EndpointClientHello {
    pub fn supports_required_codecs(&self) -> bool {
        self.snapshot_codecs
            .iter()
            .any(|codec| codec == SNAPSHOT_CODEC_V1)
            && self
                .surface_codecs
                .iter()
                .any(|codec| codec == SURFACE_CODEC_V1)
            && self
                .input_codecs
                .iter()
                .any(|codec| codec == INPUT_CODEC_V1)
            && self.blob_codecs.iter().any(|codec| codec == BLOB_CODEC_V1)
    }

    /// Whether this client also accepts the popup metrics control that rides
    /// beside the generation-1 surface frames.
    pub fn supports_surface_v2(&self) -> bool {
        self.surface_codecs
            .iter()
            .any(|codec| codec == SURFACE_CODEC_V2)
    }
}

impl EndpointServerWelcome {
    /// Builds the welcome, selecting the surface codec the client offered.
    ///
    /// `shell.surface.v1` stays the floor: a client that offers only v1 keeps
    /// exactly the generation-1 stream.
    pub fn compatible_with_surface_codec(methods: Vec<String>, surface_v2: bool) -> Self {
        Self {
            generation: ENDPOINT_PROTOCOL_GENERATION,
            server_version: crate::build_info::version(),
            snapshot_codec: SNAPSHOT_CODEC_V1.into(),
            surface_codec: if surface_v2 {
                SURFACE_CODEC_V2.into()
            } else {
                SURFACE_CODEC_V1.into()
            },
            input_codec: INPUT_CODEC_V1.into(),
            blob_codec: BLOB_CODEC_V1.into(),
            methods,
            capabilities: vec![
                SURFACE_INTEREST_CAPABILITY.into(),
                PRESENTATION_EFFECTS_FENCE_CAPABILITY.into(),
                HEALTH_CHECK_CAPABILITY.into(),
            ],
            error: None,
        }
    }

    pub fn incompatible(code: &str, message: impl Into<String>) -> Self {
        Self {
            generation: ENDPOINT_PROTOCOL_GENERATION,
            server_version: crate::build_info::version(),
            snapshot_codec: SNAPSHOT_CODEC_V1.into(),
            surface_codec: SURFACE_CODEC_V1.into(),
            input_codec: INPUT_CODEC_V1.into(),
            blob_codec: BLOB_CODEC_V1.into(),
            methods: Vec::new(),
            capabilities: Vec::new(),
            error: Some(EndpointHandshakeError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> EndpointClientHello {
        EndpointClientHello {
            generation: ENDPOINT_PROTOCOL_GENERATION,
            cell_width_px: 8,
            cell_height_px: 16,
            surface_size: ClientSurfaceSize { cols: 80, rows: 24 },
            pixel_mouse: true,
            direct_graphics: false,
            endpoint_keybindings: false,
            mouse_capture: true,
            surface_active: true,
            snapshot_codecs: vec![SNAPSHOT_CODEC_V1.into()],
            surface_codecs: vec![SURFACE_CODEC_V1.into()],
            input_codecs: vec![INPUT_CODEC_V1.into()],
            blob_codecs: vec![BLOB_CODEC_V1.into()],
        }
    }

    fn snapshot() -> ClientShellSnapshot {
        ClientShellSnapshot {
            boot_id: "boot".into(),
            revision: 1,
            config_diagnostic: None,
            product_announcement: None,
            update_available: None,
            update_install_command: "herdr update".into(),
            server_keybindings_toml: None,
            latest_release_notes_available: false,
            integration_updates_available: false,
            worktree_directory: String::new(),
            release_notes: None,
            focused_workspace_id: None,
            focused_tab_id: None,
            focused_pane_id: None,
            tab_bar_right: Vec::new(),
            tab_bar_right_separator: String::new(),
            agent_view_label: None,
            agent_order: Vec::new(),
            workspaces: Vec::new(),
            tabs: Vec::new(),
            panes: Vec::new(),
            agents: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[test]
    fn hello_ignores_future_named_fields() {
        let mut value = serde_json::to_value(hello()).unwrap();
        value["future_feature"] = serde_json::json!({"enabled": true});
        let decoded: EndpointClientHello = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, hello());
    }

    #[test]
    fn frozen_generation_one_handshake_decodes() {
        let hello: EndpointClientHello = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/endpoint-hello-v1.json"
        )))
        .unwrap();
        assert!(hello.supports_required_codecs());

        let welcome: EndpointServerWelcome = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/endpoint-welcome-v1.json"
        )))
        .unwrap();
        assert_eq!(welcome.generation, ENDPOINT_PROTOCOL_GENERATION);
        assert_eq!(welcome.snapshot_codec, SNAPSHOT_CODEC_V1);
        assert_eq!(welcome.surface_codec, SURFACE_CODEC_V1);
        assert_eq!(welcome.input_codec, INPUT_CODEC_V1);
        assert_eq!(welcome.blob_codec, BLOB_CODEC_V1);
        assert!(welcome.capabilities.is_empty());
    }

    #[test]
    fn frozen_generation_one_snapshot_decodes() {
        let snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/endpoint-snapshot-v1.json"
        )))
        .unwrap();
        assert_eq!(snapshot.boot_id, "boot-v1");
        assert_eq!(
            snapshot.workspaces[0].agent_status,
            crate::api::schema::AgentStatus::Unknown
        );
    }

    #[test]
    fn snapshot_message_uses_named_json_control() {
        let snapshot = snapshot();
        let ServerMessage::EndpointControl { kind, data } = snapshot_message(&snapshot).unwrap()
        else {
            panic!("snapshot should use endpoint control");
        };
        assert_eq!(kind, ENDPOINT_SNAPSHOT_KIND);
        let decoded: ClientShellSnapshot = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn snapshot_json_tolerates_future_fields_and_command_actions() {
        let mut snapshot = match snapshot_message(&snapshot()).unwrap() {
            ServerMessage::EndpointControl { data, .. } => {
                serde_json::from_str::<serde_json::Value>(&data).unwrap()
            }
            _ => unreachable!(),
        };
        snapshot["future_projection"] = serde_json::json!({"enabled": true});
        snapshot["commands"] = serde_json::json!([{
            "command_id": "future",
            "binding_label": "x",
            "binding_labels": ["x"],
            "action": "FutureAction",
            "description": null
        }]);

        let decoded: ClientShellSnapshot = serde_json::from_value(snapshot).unwrap();
        assert_eq!(
            decoded.commands[0].action,
            crate::protocol::ClientShellCommandAction::Unknown
        );
    }

    #[test]
    fn legacy_hello_defaults_to_an_active_surface() {
        let mut value = serde_json::to_value(hello()).unwrap();
        value.as_object_mut().unwrap().remove("surface_active");
        let decoded: EndpointClientHello = serde_json::from_value(value).unwrap();
        assert!(decoded.surface_active);
    }

    #[test]
    fn compatible_server_advertises_endpoint_lifecycle_capabilities() {
        let welcome = EndpointServerWelcome::compatible_with_surface_codec(Vec::new(), false);
        assert_eq!(
            welcome.capabilities,
            vec![
                SURFACE_INTEREST_CAPABILITY.to_string(),
                PRESENTATION_EFFECTS_FENCE_CAPABILITY.to_string(),
                HEALTH_CHECK_CAPABILITY.to_string(),
            ]
        );
    }

    #[test]
    fn required_codecs_are_explicit() {
        let mut value = hello();
        assert!(value.supports_required_codecs());
        value.snapshot_codecs.clear();
        assert!(!value.supports_required_codecs());

        let mut value = hello();
        value.surface_codecs.clear();
        assert!(!value.supports_required_codecs());

        let mut value = hello();
        value.input_codecs.clear();
        assert!(!value.supports_required_codecs());

        let mut value = hello();
        value.blob_codecs.clear();
        assert!(!value.supports_required_codecs());
    }

    #[test]
    fn generation_one_clients_keep_the_v1_surface_codec() {
        let hello = hello();
        assert!(hello.supports_required_codecs());
        assert!(!hello.supports_surface_v2());
        let welcome = EndpointServerWelcome::compatible_with_surface_codec(
            Vec::new(),
            hello.supports_surface_v2(),
        );
        assert_eq!(welcome.surface_codec, SURFACE_CODEC_V1);
    }

    #[test]
    fn surface_v2_is_selected_only_when_the_client_offers_it() {
        let mut hello = hello();
        hello.surface_codecs = vec![SURFACE_CODEC_V2.into(), SURFACE_CODEC_V1.into()];
        assert!(hello.supports_required_codecs());
        assert!(hello.supports_surface_v2());
        let welcome = EndpointServerWelcome::compatible_with_surface_codec(
            Vec::new(),
            hello.supports_surface_v2(),
        );
        assert_eq!(welcome.surface_codec, SURFACE_CODEC_V2);

        // Offering only v2 is not a compatible core: v1 stays the floor.
        let mut v2_only = hello.clone();
        v2_only.surface_codecs = vec![SURFACE_CODEC_V2.into()];
        assert!(!v2_only.supports_required_codecs());
    }

    #[test]
    fn popup_surface_metrics_round_trip_and_tolerate_missing_scroll() {
        let metrics = PopupSurfaceMetrics {
            surface_revision: 4,
            terminal_id: "term_1".into(),
            content_revision: 8,
            alternate_screen_active: false,
            scroll: Some(crate::protocol::PaneSurfaceScrollMetrics {
                offset_from_bottom: 2,
                max_offset_from_bottom: 9,
                viewport_rows: 5,
            }),
        };
        let encoded = serde_json::to_string(&metrics).unwrap();
        let decoded: PopupSurfaceMetrics = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, metrics);

        let without_scroll: PopupSurfaceMetrics = serde_json::from_str(
            r#"{"surface_revision":1,"terminal_id":"term_1","content_revision":0}"#,
        )
        .unwrap();
        assert!(without_scroll.scroll.is_none());
    }

    #[test]
    fn welcome_ignores_future_named_fields() {
        let welcome =
            EndpointServerWelcome::compatible_with_surface_codec(vec!["pane.close".into()], false);
        let mut value = serde_json::to_value(&welcome).unwrap();
        value["future_service"] = serde_json::json!("v2");
        let decoded: EndpointServerWelcome = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, welcome);
    }
}

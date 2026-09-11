use crate::api::schema::{
    PaneScrollInfo, PopupCopyMotionParams, PopupCopySearchParams, PopupScrollParams,
    PopupSelectionReadParams, ResponseResult,
};
use crate::app::App;
use crate::layout::PaneId;

use super::panes::{copy_motion_for, copy_search_for, selection_text_for};
use super::responses::{encode_error, encode_success};

fn popup_not_found(id: String, terminal_id: &str) -> String {
    encode_error(
        id,
        "popup_not_found",
        format!("popup terminal {terminal_id} not found"),
    )
}

impl App {
    /// Resolves the live popup terminal a client addressed by terminal id.
    ///
    /// The popup pane belongs to no workspace, so the workspace pane lookup
    /// cannot reach it. The terminal id is matched exactly so a request aimed
    /// at a replaced popup fails instead of acting on the current one.
    fn popup_copy_target(
        &self,
        terminal_id: &str,
    ) -> Option<(PaneId, &crate::terminal::TerminalRuntime)> {
        let popup = self.state.popup_pane.as_ref()?;
        if popup.terminal_id.as_str() != terminal_id {
            return None;
        }
        let runtime = self.terminal_runtimes.get(&popup.terminal_id)?;
        Some((popup.pane_id, runtime))
    }

    pub(super) fn handle_popup_scroll(&mut self, id: String, params: PopupScrollParams) -> String {
        let Some((_, runtime)) = self.popup_copy_target(&params.terminal_id) else {
            return popup_not_found(id, &params.terminal_id);
        };
        runtime.set_scroll_offset_from_bottom(
            usize::try_from(params.offset_from_bottom).unwrap_or(usize::MAX),
        );
        let Some(metrics) = runtime.scroll_metrics() else {
            return encode_error(
                id,
                "popup_scroll_unavailable",
                "popup scroll metrics are unavailable",
            );
        };
        encode_success(
            id,
            ResponseResult::PopupScroll {
                terminal_id: params.terminal_id,
                scroll: PaneScrollInfo {
                    offset_from_bottom: metrics.offset_from_bottom as u64,
                    max_offset_from_bottom: metrics.max_offset_from_bottom as u64,
                    viewport_rows: metrics.viewport_rows as u64,
                },
            },
        )
    }

    pub(super) fn handle_popup_selection_read(
        &mut self,
        id: String,
        params: PopupSelectionReadParams,
    ) -> String {
        let Some((pane_id, runtime)) = self.popup_copy_target(&params.terminal_id) else {
            return popup_not_found(id, &params.terminal_id);
        };
        match selection_text_for(
            runtime,
            pane_id,
            params.anchor,
            params.cursor,
            params.content_revision,
        ) {
            Ok(text) => encode_success(
                id,
                ResponseResult::PopupSelection {
                    terminal_id: params.terminal_id,
                    text,
                },
            ),
            Err((code, message)) => encode_error(id, code, message),
        }
    }

    pub(super) fn handle_popup_copy_motion(
        &mut self,
        id: String,
        params: PopupCopyMotionParams,
    ) -> String {
        let Some((pane_id, runtime)) = self.popup_copy_target(&params.terminal_id) else {
            return popup_not_found(id, &params.terminal_id);
        };
        match copy_motion_for(
            runtime,
            pane_id,
            params.cursor,
            params.motion,
            params.content_revision,
        ) {
            Ok((cursor, content_revision)) => encode_success(
                id,
                ResponseResult::PopupCopyMotion {
                    terminal_id: params.terminal_id,
                    cursor,
                    content_revision,
                },
            ),
            Err((code, message)) => encode_error(id, code, message),
        }
    }

    pub(super) fn handle_popup_copy_search(
        &mut self,
        id: String,
        params: PopupCopySearchParams,
    ) -> String {
        let Some((_, runtime)) = self.popup_copy_target(&params.terminal_id) else {
            return popup_not_found(id, &params.terminal_id);
        };
        match copy_search_for(
            runtime,
            &params.query,
            params.direction,
            params.cursor,
            params.content_revision,
            params.previous,
        ) {
            Ok(outcome) => encode_success(
                id,
                ResponseResult::PopupCopySearch {
                    terminal_id: params.terminal_id,
                    content_revision: outcome.content_revision,
                    matches: outcome.matches,
                    total: outcome.total,
                    current: outcome.current,
                    current_global: outcome.current_global,
                },
            ),
            Err((code, message)) => encode_error(id, code, message),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::schema::{
        ErrorResponse, Method, PaneTextPoint, PopupSelectionReadParams, Request, ResponseResult,
        SuccessResponse,
    };
    use crate::app::App;

    fn app_with_popup_scrollback() -> (App, String) {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            crate::app::AppPolicy::TEST,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = vec![crate::workspace::Workspace::test_new("popup")];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.ensure_test_terminals();
        let lines = (0..12)
            .map(|line| format!("popup {line:02}\n"))
            .collect::<String>();
        let runtime = crate::terminal::TerminalRuntime::test_with_scrollback_bytes(
            20,
            4,
            1000,
            lines.as_bytes(),
        );
        let (_, terminal_id) = app.install_test_popup_runtime(runtime);
        (app, terminal_id.to_string())
    }

    /// The prefix reaches the shell while a popup is open, so every prefix action
    /// is now reachable with one open. The popup lives outside the workspace tree,
    /// so none of them may corrupt state or strand it.
    #[tokio::test]
    async fn workspace_actions_stay_sound_while_a_popup_is_open() {
        use crate::api::schema::{
            EmptyParams, PaneSplitParams, PaneZoomMode, PaneZoomParams, SplitDirection,
            TabCreateParams, TabTarget, WorkspaceCloseParams, WorkspaceCreateParams,
            WorkspaceTarget,
        };
        use std::collections::HashMap;

        let split = |target_pane_id: Option<String>, direction| {
            Method::PaneSplit(PaneSplitParams {
                workspace_id: None,
                target_pane_id,
                direction,
                ratio: None,
                cwd: None,
                focus: false,
                right_click: crate::api::schema::PaneRightClickTarget::default(),
                env: HashMap::new(),
            })
        };

        let (mut app, terminal_id) = app_with_popup_scrollback();
        let mut request_id = 0;
        let mut invoke = |app: &mut App, method: Method| {
            request_id += 1;
            let response = app.handle_api_request(Request {
                id: format!("req_{request_id}"),
                method,
            });
            app.state.assert_invariants_for_test();
            for workspace in &app.state.workspaces {
                workspace.assert_invariants_for_test();
            }
            response
        };

        let pane_id = app
            .session_snapshot()
            .focused_pane_id
            .expect("focused pane");
        invoke(
            &mut app,
            split(Some(pane_id.clone()), SplitDirection::Right),
        );
        invoke(&mut app, split(None, SplitDirection::Down));
        invoke(
            &mut app,
            Method::PaneZoom(PaneZoomParams {
                pane_id: None,
                mode: PaneZoomMode::Toggle,
            }),
        );
        invoke(
            &mut app,
            Method::TabCreate(TabCreateParams {
                workspace_id: None,
                cwd: None,
                focus: true,
                label: None,
                env: HashMap::new(),
            }),
        );
        let snapshot = app.session_snapshot();
        let first_tab = snapshot.tabs.first().expect("a tab").tab_id.clone();
        invoke(
            &mut app,
            Method::TabFocus(TabTarget {
                tab_id: first_tab.clone(),
            }),
        );
        invoke(&mut app, Method::TabClose(TabTarget { tab_id: first_tab }));
        invoke(
            &mut app,
            Method::WorkspaceCreate(WorkspaceCreateParams {
                source_workspace_id: None,
                cwd: None,
                focus: true,
                label: None,
                env: HashMap::new(),
            }),
        );
        let workspaces = app.session_snapshot().workspaces;
        assert!(workspaces.len() > 1, "the focus step needs two workspaces");
        for workspace in &workspaces {
            invoke(
                &mut app,
                Method::WorkspaceFocus(WorkspaceTarget {
                    workspace_id: workspace.workspace_id.clone(),
                }),
            );
        }
        for workspace in workspaces {
            invoke(
                &mut app,
                Method::WorkspaceClose(WorkspaceCloseParams {
                    workspace_id: workspace.workspace_id,
                    close_group: false,
                }),
            );
        }

        // Every workspace is gone and the popup is still the popup: it is session
        // state, not tab state, and nothing above silently dropped or replaced it.
        assert!(app.state.workspaces.is_empty());
        assert_eq!(
            app.state
                .popup_pane
                .as_ref()
                .map(|popup| popup.terminal_id.to_string()),
            Some(terminal_id)
        );
        assert!(app.close_popup_pane());
        assert!(app.state.popup_pane.is_none());
        app.state.assert_invariants_for_test();

        let response = app.handle_api_request(Request {
            id: "req_after_close".into(),
            method: Method::PopupClose(EmptyParams::default()),
        });
        let error: ErrorResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(error.error.code, "popup_not_open");
    }

    #[tokio::test]
    async fn popup_selection_read_resolves_the_active_popup() {
        let (mut app, terminal_id) = app_with_popup_scrollback();

        let response = app.handle_api_request(Request {
            id: "req".into(),
            method: Method::PopupSelectionRead(PopupSelectionReadParams {
                terminal_id: terminal_id.clone(),
                anchor: PaneTextPoint { row: 0, col: 0 },
                cursor: PaneTextPoint { row: 0, col: 7 },
                content_revision: None,
            }),
        });

        let success: SuccessResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(
            success.result,
            ResponseResult::PopupSelection {
                terminal_id,
                text: "popup 00".into(),
            }
        );
    }

    #[tokio::test]
    async fn popup_selection_read_rejects_a_replaced_popup() {
        let (mut app, _) = app_with_popup_scrollback();

        let response = app.handle_api_request(Request {
            id: "req".into(),
            method: Method::PopupSelectionRead(PopupSelectionReadParams {
                terminal_id: "term_other".into(),
                anchor: PaneTextPoint { row: 0, col: 0 },
                cursor: PaneTextPoint { row: 0, col: 7 },
                content_revision: None,
            }),
        });

        let error: ErrorResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(error.error.code, "popup_not_found");
    }

    #[tokio::test]
    async fn popup_selection_read_fails_when_no_popup_is_open() {
        let (mut app, terminal_id) = app_with_popup_scrollback();
        app.state.popup_pane = None;

        let response = app.handle_api_request(Request {
            id: "req".into(),
            method: Method::PopupSelectionRead(PopupSelectionReadParams {
                terminal_id,
                anchor: PaneTextPoint { row: 0, col: 0 },
                cursor: PaneTextPoint { row: 0, col: 7 },
                content_revision: None,
            }),
        });

        let error: ErrorResponse = serde_json::from_str(&response).unwrap();
        assert_eq!(error.error.code, "popup_not_found");
    }

    #[tokio::test]
    async fn popup_scroll_moves_the_popup_terminal_and_reports_metrics() {
        let (mut app, terminal_id) = app_with_popup_scrollback();

        let response = app.handle_api_request(Request {
            id: "req".into(),
            method: Method::PopupScroll(crate::api::schema::PopupScrollParams {
                terminal_id: terminal_id.clone(),
                offset_from_bottom: 3,
            }),
        });

        let success: SuccessResponse = serde_json::from_str(&response).unwrap();
        let ResponseResult::PopupScroll {
            terminal_id: returned,
            scroll,
        } = success.result
        else {
            panic!("popup scroll should report metrics");
        };
        assert_eq!(returned, terminal_id);
        assert_eq!(scroll.offset_from_bottom, 3);
        assert!(scroll.max_offset_from_bottom >= 3);
    }

    #[tokio::test]
    async fn popup_copy_motion_targets_the_popup_terminal() {
        let (mut app, terminal_id) = app_with_popup_scrollback();

        let response = app.handle_api_request(Request {
            id: "req".into(),
            method: Method::PopupCopyMotion(crate::api::schema::PopupCopyMotionParams {
                terminal_id: terminal_id.clone(),
                cursor: PaneTextPoint { row: 0, col: 0 },
                motion: crate::api::schema::PaneCopyMotion::LineEnd,
                content_revision: None,
            }),
        });

        let success: SuccessResponse = serde_json::from_str(&response).unwrap();
        let ResponseResult::PopupCopyMotion {
            terminal_id: returned,
            cursor,
            ..
        } = success.result
        else {
            panic!("popup copy motion should report a cursor");
        };
        assert_eq!(returned, terminal_id);
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 7);
    }
}

use super::*;

fn restore_mode_bar(
    frame: &mut FrameData,
    bar: Option<Rect>,
    cells: Option<&[crate::protocol::CellData]>,
) {
    let (Some(bar), Some(cells)) = (bar, cells) else {
        return;
    };
    let start = usize::from(bar.y) * usize::from(frame.width) + usize::from(bar.x);
    frame.cells[start..start + usize::from(bar.width)].clone_from_slice(cells);
    if frame
        .cursor
        .as_ref()
        .is_some_and(|cursor| cursor.y == bar.y)
    {
        frame.cursor = None;
    }
}

impl ClientShellState {
    fn compose_unavailable(&mut self, cols: u16, rows: u16) -> FrameData {
        let layout = self.layout(cols, rows);
        let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
        buffer.set_style(
            buffer.area,
            Style::default()
                .fg(self.config.palette.text)
                .bg(self.config.palette.panel_bg),
        );
        self.hits = ShellHitMap::default();
        let sidebar = if layout.sidebar.width > 0 {
            layout.sidebar
        } else {
            Rect::new(0, 1, cols, rows.saturating_sub(2))
        };
        let valid_navigation_target = self.mode == ClientShellMode::Navigate
            && self
                .navigate_workspace_id
                .as_ref()
                .is_some_and(|target| self.navigation_target_valid(target));
        super::endpoint_sidebar::render_expanded(
            &mut buffer,
            sidebar,
            self.snapshot.as_deref(),
            &self.config,
            &mut render::ShellRenderState {
                endpoints: &self.endpoints,
                active_endpoint_id: &self.active_endpoint_id,
                collapsed_endpoints: &self.collapsed_endpoints,
                collapsed_groups: &self.collapsed_groups,
                remote_collapsed_groups: &self.remote_collapsed_groups,
                workspace_scroll: &mut self.workspace_scroll,
                agent_scroll: &mut self.agent_scroll,
                tab_scroll: &mut self.tab_scroll,
                reveal_focused_workspace: &mut self.reveal_focused_workspace,
                reveal_focused_tab: &mut self.reveal_focused_tab,
                sidebar_collapsed: false,
                sidebar_section_split: self.sidebar_section_split,
                tab_drag_insert_index: None,
                prefix_highlight: false,
                selected_workspace_id: self
                    .navigate_workspace_id
                    .as_ref()
                    .filter(|_| valid_navigation_target),
                reveal_navigation_workspace: &mut self.reveal_navigation_workspace,
                dragged_workspace_id: None,
                workspace_drop_indicator_row: None,
            },
            &mut self.hits,
        );
        if !self.config.mouse_capture {
            self.hits = ShellHitMap::default();
        }
        let message = self.endpoint_error.clone().unwrap_or_else(|| {
            let status = self
                .endpoint_status(&self.active_endpoint_id)
                .unwrap_or(ClientEndpointStatus::Connecting);
            let (_, label, _) = endpoint_status_presentation(status, &self.config.palette);
            format!(
                "{}: {label}. Select a connected machine.",
                self.active_endpoint_label()
            )
        });
        let message_area = if layout.sidebar.width > 0 {
            layout.pane_surface
        } else {
            Rect::new(0, 0, cols, 1)
        };
        render::put_text(
            &mut buffer,
            message_area.x,
            message_area.y,
            message_area.width,
            &message,
            Style::default().fg(self.config.palette.overlay0),
        );
        render::render_mode_bar(
            &mut buffer,
            Rect::new(0, 0, cols, rows),
            self.mode,
            None,
            self.endpoint_error.as_deref(),
            false,
            &self.config.keybinds,
            &self.config.palette,
        );
        FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[])
    }

    /// True when prefix mode should be signalled by recoloring the focused pane
    /// border and the active tab instead of drawing the bottom hint bar.
    /// Driven by `[ui] prefix_indicator = "highlight"`.
    pub(super) fn prefix_highlight_active(&self) -> bool {
        self.mode == ClientShellMode::Prefix
            && self.config.prefix_indicator == PrefixIndicatorConfig::Highlight
    }

    /// Paints the mouse selection and copy-mode search highlights that belong to
    /// one pane or popup hit.
    fn render_hit_selection_highlights(
        &self,
        composed: &mut ratatui::buffer::Buffer,
        hit: &PaneHit,
    ) {
        let copy_surface_coherent = client_copy_surface_coherent(self.copy_mode.as_ref(), hit);
        if copy_surface_coherent {
            render_client_copy_search_highlights(
                composed,
                self.copy_mode.as_ref(),
                hit,
                &self.config.palette,
                false,
            );
        }
        let selection_is_stale_copy_projection = !copy_surface_coherent
            && self.copy_mode.as_ref().is_some_and(|copy_mode| {
                copy_mode.pane_id == hit.pane_id
                    && self
                        .selection
                        .as_ref()
                        .is_some_and(|selection| selection.pane_id == hit.pane_id)
            });
        if !selection_is_stale_copy_projection {
            crate::ui::render_selection_highlight(
                self.selection.as_ref(),
                composed,
                &hit.pane_id,
                hit.inner_rect,
                hit.scroll,
                &self.config.palette,
                crate::terminal_theme::TerminalTheme::default(),
            );
        }
        if copy_surface_coherent {
            render_client_copy_search_highlights(
                composed,
                self.copy_mode.as_ref(),
                hit,
                &self.config.palette,
                true,
            );
        }
    }

    /// Paints the copy-mode block cursor inside one pane or popup hit.
    fn render_hit_copy_cursor(&self, frame: &mut FrameData, hit: &PaneHit) -> Option<()> {
        let copy_mode = self.copy_mode.as_ref()?;
        let viewport_top = copy_mode
            .max_offset_from_bottom
            .saturating_sub(copy_mode.offset_from_bottom)
            .min(u32::MAX as usize) as u32;
        let viewport_row = copy_mode.cursor.row.saturating_sub(viewport_top);
        if viewport_row >= u32::from(hit.inner_rect.height)
            || copy_mode.cursor.col >= hit.inner_rect.width
        {
            frame.cursor = None;
            return Some(());
        }
        let mut composed = frame.to_ratatui_buffer()?;
        let x = hit.inner_rect.x + copy_mode.cursor.col;
        let y = hit.inner_rect.y + viewport_row as u16;
        composed[(x, y)].set_style(
            Style::default()
                .fg(match self.config.palette.panel_bg {
                    ratatui::style::Color::Reset => self.config.palette.surface_dim,
                    color => color,
                })
                .bg(self.config.palette.accent)
                .add_modifier(Modifier::BOLD),
        );
        frame.replace_from_ratatui_buffer_preserving_effects(&composed, None);
        Some(())
    }

    pub(crate) fn compose(&mut self, cols: u16, rows: u16) -> Option<FrameData> {
        if self.last_composed_size != Some((cols, rows)) && self.mode == ClientShellMode::Navigate {
            self.reveal_navigation_workspace = true;
            self.reveal_mobile_workspace = true;
        }
        self.last_composed_size = Some((cols, rows));
        let valid_navigation_target = self.mode == ClientShellMode::Navigate
            && self
                .navigate_workspace_id
                .as_ref()
                .is_some_and(|target| self.navigation_target_valid(target));
        if self.snapshot.is_none() || self.pane_surface.is_none() {
            return Some(self.compose_unavailable(cols, rows));
        }
        let snapshot = self.snapshot.as_deref()?;
        // A one-step successor is retained separately until its exact snapshot arrives; do not
        // keep composing the now-superseded current pair while it is pending.
        if self.pending_pane_surface.is_some() {
            return None;
        }
        let surface = self.pane_surface.as_ref()?;
        if snapshot.revision != surface.projection_revision {
            return None;
        }
        let layout = self.layout(cols, rows);
        if self.last_tab_bar_width != Some(layout.tab_bar.width) {
            self.last_tab_bar_width = Some(layout.tab_bar.width);
            self.reveal_focused_tab = true;
        }
        let tab_drag_insert_index = match &self.chrome_drag {
            Some(ClientChromeDrag::Tab { insert_index, .. }) => *insert_index,
            _ => None,
        };
        let prefix_highlight = self.prefix_highlight_active();
        let (dragged_workspace_id, workspace_drop_indicator_row) = match &self.chrome_drag {
            Some(ClientChromeDrag::Workspace {
                source_workspace_id,
                target,
            }) => (
                Some(source_workspace_id.as_str()),
                target.as_ref().map(|(_, row)| *row),
            ),
            _ => (None, None),
        };
        let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
        self.hits = render::render_shell(
            &mut buffer,
            layout,
            snapshot,
            &self.config,
            render::ShellRenderState {
                endpoints: &self.endpoints,
                active_endpoint_id: &self.active_endpoint_id,
                collapsed_endpoints: &self.collapsed_endpoints,
                collapsed_groups: &self.collapsed_groups,
                remote_collapsed_groups: &self.remote_collapsed_groups,
                workspace_scroll: &mut self.workspace_scroll,
                agent_scroll: &mut self.agent_scroll,
                tab_scroll: &mut self.tab_scroll,
                reveal_focused_workspace: &mut self.reveal_focused_workspace,
                reveal_focused_tab: &mut self.reveal_focused_tab,
                sidebar_collapsed: self.sidebar_collapsed,
                sidebar_section_split: self.sidebar_section_split,
                tab_drag_insert_index,
                prefix_highlight,
                selected_workspace_id: self
                    .navigate_workspace_id
                    .as_ref()
                    .filter(|_| valid_navigation_target),
                reveal_navigation_workspace: &mut self.reveal_navigation_workspace,
                dragged_workspace_id,
                workspace_drop_indicator_row,
            },
        );
        self.hits.panes = surface
            .panes
            .iter()
            .map(|pane| PaneHit {
                rect: Rect::new(
                    layout.pane_surface.x.saturating_add(pane.rect.x),
                    layout.pane_surface.y.saturating_add(pane.rect.y),
                    pane.rect.width,
                    pane.rect.height,
                ),
                inner_rect: Rect::new(
                    layout.pane_surface.x.saturating_add(pane.inner_rect.x),
                    layout.pane_surface.y.saturating_add(pane.inner_rect.y),
                    pane.inner_rect.width,
                    pane.inner_rect.height,
                ),
                scrollbar_rect: pane.scrollbar_rect.map(|rect| {
                    Rect::new(
                        layout.pane_surface.x.saturating_add(rect.x),
                        layout.pane_surface.y.saturating_add(rect.y),
                        rect.width,
                        rect.height,
                    )
                }),
                scroll: pane.scroll.map(|metrics| crate::pane::ScrollMetrics {
                    offset_from_bottom: usize::try_from(metrics.offset_from_bottom)
                        .unwrap_or(usize::MAX),
                    max_offset_from_bottom: usize::try_from(metrics.max_offset_from_bottom)
                        .unwrap_or(usize::MAX),
                    viewport_rows: usize::try_from(metrics.viewport_rows).unwrap_or(usize::MAX),
                }),
                pane_id: pane.pane_id.clone(),
                popup: false,
                mouse_reporting: pane.mouse_reporting,
                sgr_pixel_mouse: pane.sgr_pixel_mouse,
                pixel_width: pane.pixel_width,
                pixel_height: pane.pixel_height,
            })
            .collect();
        let topology_signature = pane_surface_topology_signature(surface);
        self.hits.pane_splits = surface
            .splits
            .iter()
            .map(|split| PaneSplitHit {
                direction: split.direction,
                pos: match split.direction {
                    crate::protocol::PaneSurfaceSplitDirection::Horizontal => {
                        layout.pane_surface.x.saturating_add(split.pos)
                    }
                    crate::protocol::PaneSurfaceSplitDirection::Vertical => {
                        layout.pane_surface.y.saturating_add(split.pos)
                    }
                },
                area: Rect::new(
                    layout.pane_surface.x.saturating_add(split.area.x),
                    layout.pane_surface.y.saturating_add(split.area.y),
                    split.area.width,
                    split.area.height,
                ),
                hit_rect: Rect::new(
                    layout.pane_surface.x.saturating_add(split.hit_rect.x),
                    layout.pane_surface.y.saturating_add(split.hit_rect.y),
                    split.hit_rect.width,
                    split.hit_rect.height,
                ),
                path: split.path.clone(),
                topology_signature,
            })
            .collect();
        if !self.config.mouse_capture {
            self.hits.pane_splits.clear();
        }
        let mode_bar_area = if layout.mobile_header.is_empty()
            && self.config.tab_bar_position == TabBarPositionConfig::Bottom
            && !layout.tab_bar.is_empty()
        {
            layout.tab_bar
        } else {
            layout.pane_surface
        };
        let mobile_navigate_panel = !layout.mobile_header.is_empty()
            && self.mode == ClientShellMode::Navigate
            && self.endpoint_error.is_none();
        let mode_bar = if mobile_navigate_panel
            || self.overlay.is_some()
            || (prefix_highlight && self.endpoint_error.is_none())
        {
            None
        } else {
            render::render_mode_bar(
                &mut buffer,
                mode_bar_area,
                self.mode,
                self.copy_mode.as_ref(),
                self.endpoint_error.as_deref(),
                snapshot.update_available.is_some(),
                &self.config.keybinds,
                &self.config.palette,
            )
        };
        if mode_bar == Some(layout.tab_bar) {
            self.hits.tabs.clear();
            self.hits.new_tab = Rect::default();
            self.hits.tab_scroll_left = Rect::default();
            self.hits.tab_scroll_right = Rect::default();
        }
        let mut frame = FrameData::from_ratatui_buffer_with_hyperlinks(&buffer, None, &[]);
        let mode_bar_cells = mode_bar.map(|bar| {
            let start = usize::from(bar.y) * usize::from(frame.width) + usize::from(bar.x);
            frame.cells[start..start + usize::from(bar.width)].to_vec()
        });
        blit_pane_surface(&mut frame, &surface.frame, layout.pane_surface);
        if prefix_highlight {
            highlight_focused_pane_border(
                &mut frame,
                surface,
                layout.pane_surface,
                crate::protocol::color_to_u32(self.config.palette.accent),
                crate::protocol::color_to_u32(self.config.palette.yellow),
            );
        }
        restore_mode_bar(&mut frame, mode_bar, mode_bar_cells.as_deref());
        let has_selection = self
            .selection
            .as_ref()
            .is_some_and(|selection| selection.is_visible());
        let has_search = self
            .copy_mode
            .as_ref()
            .is_some_and(|copy_mode| !copy_mode.search_matches.is_empty());
        if has_selection || has_search {
            let cursor = frame.cursor.clone();
            let mut composed = frame.to_ratatui_buffer()?;
            for hit in &self.hits.panes {
                self.render_hit_selection_highlights(&mut composed, hit);
            }
            frame.replace_from_ratatui_buffer_preserving_effects(&composed, cursor);
        }
        if self.mode == ClientShellMode::Copy {
            frame.cursor = None;
            if let Some(hit) = self.copy_mode.as_ref().and_then(|copy_mode| {
                self.hits.panes.iter().find(|hit| {
                    hit.pane_id == copy_mode.pane_id
                        && client_copy_surface_coherent(Some(copy_mode), hit)
                })
            }) {
                self.render_hit_copy_cursor(&mut frame, &hit.clone())?;
            }
        }
        restore_mode_bar(&mut frame, mode_bar, mode_bar_cells.as_deref());
        self.hits.notification_toast = Rect::default();
        let has_config_diagnostic = self.config_diagnostic.is_some();
        let active_lifecycle = self
            .endpoints
            .iter()
            .find(|endpoint| endpoint.endpoint_id == self.active_endpoint_id)
            .filter(|endpoint| endpoint.status != ClientEndpointStatus::Online)
            .map(|endpoint| (endpoint.label.clone(), endpoint.status));
        if has_config_diagnostic
            || active_lifecycle.is_some()
            || self.visible_endpoint_notice.is_some()
            || self.visible_notification.is_some()
        {
            let cursor = frame.cursor.clone();
            let mut composed = frame.to_ratatui_buffer()?;
            if let Some(diagnostic) = self.config_diagnostic.as_deref() {
                let diagnostic_area = if layout.mobile_header.is_empty() {
                    Rect::new(0, 0, cols, rows)
                } else {
                    layout.pane_surface
                };
                crate::ui::render_config_diagnostic_buffer(
                    &mut composed,
                    diagnostic_area,
                    diagnostic,
                    &self.config.palette,
                );
            }
            let lifecycle_offset = active_lifecycle.as_ref().map_or(0, |(label, status)| {
                let _ = endpoint_notices::render_lifecycle_banner(
                    &mut composed,
                    Rect::new(0, 0, cols, rows),
                    label,
                    *status,
                    u16::from(has_config_diagnostic) + layout.mobile_header.height,
                    &self.config.palette,
                );
                1
            });
            if let Some(notice) = self.visible_endpoint_notice.as_ref() {
                self.hits.notification_toast = if layout.mobile_header.is_empty() {
                    endpoint_notices::render_notice(
                        &mut composed,
                        Rect::new(0, 0, cols, rows),
                        notice,
                        u16::from(has_config_diagnostic) + lifecycle_offset,
                        &self.config.palette,
                    )
                } else {
                    endpoint_notices::render_mobile_banner(
                        &mut composed,
                        Rect::new(0, 0, cols, rows),
                        notice,
                        has_config_diagnostic || lifecycle_offset > 0,
                        &self.config.palette,
                    )
                };
            } else if let Some(notification) = self.visible_notification.as_ref() {
                self.hits.notification_toast = if layout.mobile_header.is_empty() {
                    notifications::render_visible_notification(
                        &mut composed,
                        Rect::new(0, 0, cols, rows),
                        notification,
                        self.config.toast_position,
                        u16::from(has_config_diagnostic) + lifecycle_offset,
                        &self.config.palette,
                    )
                } else {
                    notifications::render_mobile_notification_banner(
                        &mut composed,
                        Rect::new(0, 0, cols, rows),
                        notification,
                        has_config_diagnostic || lifecycle_offset > 0,
                        &self.config.palette,
                    )
                };
            }
            frame.replace_from_ratatui_buffer_preserving_effects(&composed, cursor);
        }
        if let Some(feedback) = self.copy_feedback.as_ref() {
            let cursor = frame.cursor.clone();
            let mut composed = frame.to_ratatui_buffer()?;
            let base_offset = u16::from(has_config_diagnostic);
            let feedback_area = if layout.mobile_header.is_empty() {
                layout.pane_surface
            } else {
                Rect::new(0, 0, cols, rows)
            };
            let offset = crate::ui::copy_feedback_offset_for_toast(
                feedback_area,
                feedback,
                base_offset,
                self.config.clipboard_toast_position,
                self.hits.notification_toast,
            );
            crate::ui::render_copy_feedback_buffer(
                &mut composed,
                feedback_area,
                feedback,
                offset,
                self.config.clipboard_toast_position,
                &self.config.palette,
            );
            frame.replace_from_ratatui_buffer_preserving_effects(&composed, cursor);
        }
        self.hits.popup = None;
        if let Some(popup) = surface.popup.as_deref() {
            let width = popup.width.map(client_popup_size);
            let height = popup.height.map(client_popup_size);
            if let Some(geometry) = client_popup_geometry(
                width,
                height,
                self.config.popup_padding,
                layout.pane_surface,
                &popup.frame,
            ) {
                let mut composed = frame.to_ratatui_buffer()?;
                // This reaches only the border and the padding; the content
                // area is overwritten by the popup's own cells below, and
                // filled in afterwards when the whole popup should be solid.
                let chrome_bg = match self.config.popup_background {
                    crate::config::PopupBackground::Panel | crate::config::PopupBackground::All => {
                        self.config.palette.panel_bg
                    }
                    crate::config::PopupBackground::Transparent => ratatui::style::Color::Reset,
                };
                // Dim first: the popup's own Clear, block and content all
                // overwrite their cells afterwards, so the popup stays bright
                // without having to undim itself.
                let dim_area = match self.config.popup_dim_background {
                    crate::config::PopupDimBackground::Off => None,
                    crate::config::PopupDimBackground::All => Some(composed.area),
                    crate::config::PopupDimBackground::Panes => Some(layout.pane_surface),
                };
                if let Some(dim_area) = dim_area {
                    crate::ui::dim_buffer(&mut composed, dim_area);
                }
                let block = ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(ratatui::style::Style::default().fg(self.config.palette.accent))
                    .title(
                        crate::ui::popup_border_title(&popup.title, geometry.outer.width)
                            .unwrap_or_default(),
                    )
                    .style(ratatui::style::Style::default().bg(chrome_bg));
                ratatui::widgets::Widget::render(
                    ratatui::widgets::Clear,
                    geometry.outer,
                    &mut composed,
                );
                ratatui::widgets::Widget::render(block, geometry.outer, &mut composed);
                crate::ui::repair_wide_grapheme_edges(&mut composed, geometry.outer);
                frame.replace_from_ratatui_buffer_preserving_effects(&composed, None);
                blit_pane_surface(&mut frame, &popup.frame, geometry.inner);
                if self.config.popup_background == crate::config::PopupBackground::All {
                    fill_terminal_default_background(
                        &mut frame,
                        geometry.inner,
                        crate::protocol::color_to_u32(self.config.palette.panel_bg),
                    );
                }
                let popup_hit = PaneHit {
                    rect: geometry.outer,
                    inner_rect: geometry.inner,
                    scrollbar_rect: None,
                    scroll: self
                        .popup_metrics_for(&popup.terminal_id)
                        .and_then(|metrics| metrics.scroll)
                        .map(|metrics| crate::pane::ScrollMetrics {
                            offset_from_bottom: usize::try_from(metrics.offset_from_bottom)
                                .unwrap_or(usize::MAX),
                            max_offset_from_bottom: usize::try_from(metrics.max_offset_from_bottom)
                                .unwrap_or(usize::MAX),
                            viewport_rows: usize::try_from(metrics.viewport_rows)
                                .unwrap_or(usize::MAX),
                        }),
                    pane_id: popup.terminal_id.clone(),
                    popup: true,
                    mouse_reporting: popup.mouse_reporting,
                    sgr_pixel_mouse: popup.sgr_pixel_mouse,
                    pixel_width: popup.pixel_width,
                    pixel_height: popup.pixel_height,
                };
                // The popup's own cells were just blitted over the pane surface, so
                // its highlights and copy cursor have to be painted again on top.
                if has_selection || has_search {
                    let cursor = frame.cursor.clone();
                    let mut composed = frame.to_ratatui_buffer()?;
                    self.render_hit_selection_highlights(&mut composed, &popup_hit);
                    frame.replace_from_ratatui_buffer_preserving_effects(&composed, cursor);
                }
                if self.mode == ClientShellMode::Copy
                    && self.copy_mode.as_ref().is_some_and(|copy_mode| {
                        copy_mode.pane_id == popup_hit.pane_id
                            && client_copy_surface_coherent(Some(copy_mode), &popup_hit)
                    })
                {
                    frame.cursor = None;
                    self.render_hit_copy_cursor(&mut frame, &popup_hit)?;
                }
                self.hits.popup = Some(popup_hit);
            }
        }
        if !layout.mobile_header.is_empty()
            && self.mode == ClientShellMode::Navigate
            && self.overlay.is_none()
        {
            let mut composed = frame.to_ratatui_buffer()?;
            super::mobile::render_mobile_switcher(
                &mut composed,
                Rect::new(0, 0, cols, rows),
                snapshot,
                &self.endpoints,
                &self.active_endpoint_id,
                &self.config,
                self.navigate_workspace_id
                    .as_ref()
                    .filter(|_| valid_navigation_target),
                &mut self.mobile_switcher_scroll,
                &mut self.reveal_mobile_workspace,
                &mut self.hits,
            );
            if let Some((label, status)) = active_lifecycle.as_ref() {
                let _ = endpoint_notices::render_lifecycle_banner(
                    &mut composed,
                    Rect::new(0, 0, cols, rows),
                    label,
                    *status,
                    2,
                    &self.config.palette,
                );
            }
            if let Some(notice) = self.visible_endpoint_notice.as_ref() {
                self.hits.notification_toast = endpoint_notices::render_mobile_banner(
                    &mut composed,
                    Rect::new(0, 0, cols, rows),
                    notice,
                    active_lifecycle.is_some(),
                    &self.config.palette,
                );
            }
            frame.replace_from_ratatui_buffer_preserving_effects(&composed, None);
            self.hits.panes.clear();
            self.hits.pane_splits.clear();
            self.hits.popup = None;
        }
        restore_mode_bar(&mut frame, mode_bar, mode_bar_cells.as_deref());
        if let Some(overlay) = self.overlay.as_ref() {
            let mut composed = frame.to_ratatui_buffer()?;
            let cursor = if let ClientShellOverlay::ContextMenu(menu) = overlay {
                self.hits.context_menu_rows =
                    render::render_context_menu(&mut composed, menu, &self.config.palette)?;
                None
            } else if let ClientShellOverlay::GlobalMenu(menu) = overlay {
                self.hits.global_menu_rows = render::render_global_menu(
                    &mut composed,
                    self.hits.global_launcher,
                    menu,
                    snapshot,
                    &self.config.palette,
                )?;
                None
            } else {
                let rendered = render::render_client_overlay(
                    &mut composed,
                    overlay,
                    snapshot,
                    &self.endpoints,
                    &self.active_endpoint_id,
                    &self.config.keybinds,
                    &self.config.palette,
                )?;
                self.hits.overlay_primary = rendered.primary;
                self.hits.overlay_clear = rendered.clear;
                self.hits.overlay_cancel = rendered.cancel;
                self.hits.navigator_popup = rendered.navigator_popup;
                self.hits.navigator_search = rendered.navigator_search;
                self.hits.navigator_rows = rendered.navigator_rows;
                self.hits.worktree_search = rendered.worktree_search;
                self.hits.worktree_rows = rendered.worktree_rows;
                self.hits.help_popup = rendered.help_popup;
                self.hits.help_scrollbar = rendered.help_scrollbar;
                self.hits.help_scroll_metrics = rendered.help_scroll_metrics;
                self.hits.help_max_scroll = rendered.help_max_scroll;
                self.hits.settings_popup = rendered.settings_popup;
                self.hits.settings_tabs = rendered.settings_tabs;
                self.hits.settings_choices = rendered.settings_choices;
                self.hits.product_announcement_scrollbar = rendered.product_announcement_scrollbar;
                self.hits.product_announcement_scroll_metrics =
                    rendered.product_announcement_scroll_metrics;
                self.hits.product_announcement_max_scroll =
                    rendered.product_announcement_max_scroll;
                self.hits.release_notes_scrollbar = rendered.release_notes_scrollbar;
                self.hits.release_notes_scroll_metrics = rendered.release_notes_scroll_metrics;
                self.hits.release_notes_max_scroll = rendered.release_notes_max_scroll;
                rendered.cursor
            };
            frame.replace_from_ratatui_buffer_preserving_effects(&composed, cursor);
        }
        if let Some(ClientShellOverlay::Help(help)) = self.overlay.as_mut() {
            help.scroll = help.scroll.min(self.hits.help_max_scroll);
        }
        if let Some(ClientShellOverlay::ProductAnnouncement(announcement)) = self.overlay.as_mut() {
            announcement.scroll = announcement
                .scroll
                .min(u16::try_from(self.hits.product_announcement_max_scroll).unwrap_or(u16::MAX));
        }
        if let Some(ClientShellOverlay::ReleaseNotes(notes)) = self.overlay.as_mut() {
            notes.scroll = notes
                .scroll
                .min(u16::try_from(self.hits.release_notes_max_scroll).unwrap_or(u16::MAX));
        }
        if self.endpoint_status(&self.active_endpoint_id) != Some(ClientEndpointStatus::Online) {
            frame.cursor = None;
            self.hits.panes.clear();
            self.hits.pane_splits.clear();
            self.hits.popup = None;
        }
        self.compose_graphics(&mut frame, layout);
        Some(frame)
    }
}

fn client_copy_surface_coherent(copy_mode: Option<&ClientCopyModeState>, hit: &PaneHit) -> bool {
    copy_mode
        .filter(|copy_mode| copy_mode.pane_id == hit.pane_id)
        .is_none_or(|copy_mode| {
            copy_mode.geometry == (hit.inner_rect.width, hit.inner_rect.height)
                && hit.scroll.is_some_and(|scroll| {
                    scroll.offset_from_bottom == copy_mode.offset_from_bottom
                        && scroll.max_offset_from_bottom == copy_mode.max_offset_from_bottom
                })
        })
}

fn surface_rect_contains(rect: crate::protocol::SurfaceRect, area: Rect, x: u16, y: u16) -> bool {
    let left = area.x.saturating_add(rect.x);
    let top = area.y.saturating_add(rect.y);
    x >= left
        && x < left.saturating_add(rect.width)
        && y >= top
        && y < top.saturating_add(rect.height)
}

/// Repaints the focused pane's border ring from `from` to `to` in the composed
/// frame.
///
/// The server already draws that border and its title in the accent color, so
/// prefix highlighting only has to swap the foreground client-side; nothing
/// about prefix mode has to reach the server. The ring mirrors the server's
/// `line_touches_pane` geometry: the pane rect's own edges plus the seam column
/// and row it shares with the panes to the right and below. Content and
/// scrollbar cells are skipped, and the `from` filter leaves unfocused borders,
/// padding, and endpoints with a different palette untouched.
fn highlight_focused_pane_border(
    frame: &mut FrameData,
    surface: &PaneSurfaceFrame,
    area: Rect,
    from: u32,
    to: u32,
) {
    let Some(focused) = surface.panes.iter().find(|pane| pane.focused) else {
        return;
    };
    if focused.rect.width == 0 || focused.rect.height == 0 {
        return;
    }
    let left = area.x.saturating_add(focused.rect.x);
    let top = area.y.saturating_add(focused.rect.y);
    let right = left.saturating_add(focused.rect.width).saturating_sub(1);
    let bottom = top.saturating_add(focused.rect.height).saturating_sub(1);
    let seam_x = right.saturating_add(1);
    let seam_y = bottom.saturating_add(1);
    let frame_width = usize::from(frame.width);
    let mut recolor = |x: u16, y: u16| {
        if x < area.x
            || x >= area.right()
            || x >= frame.width
            || y < area.y
            || y >= area.bottom()
            || y >= frame.height
        {
            return;
        }
        if surface.panes.iter().any(|pane| {
            surface_rect_contains(pane.inner_rect, area, x, y)
                || pane
                    .scrollbar_rect
                    .is_some_and(|rect| surface_rect_contains(rect, area, x, y))
        }) {
            return;
        }
        let index = usize::from(y)
            .saturating_mul(frame_width)
            .saturating_add(usize::from(x));
        if let Some(cell) = frame.cells.get_mut(index) {
            if cell.fg == from {
                cell.fg = to;
            }
        }
    };
    for y in [top, bottom, seam_y] {
        for x in left..=seam_x {
            recolor(x, y);
        }
    }
    for x in [left, right, seam_x] {
        for y in top..=seam_y {
            recolor(x, y);
        }
    }
}

fn render_client_copy_search_highlights(
    buffer: &mut Buffer,
    copy_mode: Option<&ClientCopyModeState>,
    hit: &PaneHit,
    palette: &Palette,
    current_only: bool,
) {
    let Some(copy_mode) = copy_mode.filter(|copy_mode| copy_mode.pane_id == hit.pane_id) else {
        return;
    };
    if hit.inner_rect.is_empty() {
        return;
    }
    let top = copy_mode
        .max_offset_from_bottom
        .saturating_sub(copy_mode.offset_from_bottom)
        .min(u32::MAX as usize) as u32;
    let bottom = top.saturating_add(u32::from(hit.inner_rect.height.saturating_sub(1)));
    let style = if current_only {
        Style::default()
            .fg(panel_contrast_fg(palette))
            .bg(palette.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text).bg(palette.surface1)
    };
    for (index, text_match) in copy_mode.search_matches.iter().enumerate() {
        if (copy_mode.search_current == Some(index)) != current_only
            || text_match.end.row < top
            || text_match.start.row > bottom
        {
            continue;
        }
        let start_row = text_match.start.row.max(top);
        let end_row = text_match.end.row.min(bottom);
        for absolute_row in start_row..=end_row {
            let viewport_row = absolute_row.saturating_sub(top) as u16;
            let start_col = if absolute_row == text_match.start.row {
                text_match.start.col
            } else {
                0
            };
            let end_col = if absolute_row == text_match.end.row {
                text_match.end.col
            } else {
                hit.inner_rect.width.saturating_sub(1)
            };
            for col in start_col..=end_col.min(hit.inner_rect.width.saturating_sub(1)) {
                buffer[(
                    hit.inner_rect.x.saturating_add(col),
                    hit.inner_rect.y.saturating_add(viewport_row),
                )]
                    .set_style(style);
            }
        }
    }
}

/// Resolves the popup geometry this client will blit the server's frame into.
///
/// The server sizes the popup PTY from its own `[ui] popup_padding`, so a
/// remote client whose config disagrees would otherwise blit a frame that no
/// longer fits its inner rect and lose the right and bottom edges. Only a
/// frame that matches the unpadded geometry exactly proves the disagreement;
/// any other mismatch is a resize still in flight, where keeping the padded
/// geometry avoids flickering the padding off for one frame.
fn client_popup_geometry(
    width: Option<crate::popup_size::PopupSize>,
    height: Option<crate::popup_size::PopupSize>,
    padding: crate::config::PanePadding,
    area: Rect,
    frame: &FrameData,
) -> Option<crate::popup_size::PopupResolvedGeometry> {
    let fits = |geometry: &crate::popup_size::PopupResolvedGeometry| {
        geometry.inner.width == frame.width && geometry.inner.height == frame.height
    };
    let padded = crate::popup_size::resolve_popup_geometry(width, height, padding, area)?;
    if fits(&padded) {
        return Some(padded);
    }
    let unpadded = crate::popup_size::resolve_popup_geometry(
        width,
        height,
        crate::config::PanePadding::default(),
        area,
    )?;
    if fits(&unpadded) {
        return Some(unpadded);
    }
    Some(padded)
}

fn client_popup_size(size: crate::protocol::ClientShellPopupSize) -> crate::popup_size::PopupSize {
    match size {
        crate::protocol::ClientShellPopupSize::Cells(cells) => {
            crate::popup_size::PopupSize::Cells(cells)
        }
        crate::protocol::ClientShellPopupSize::Percent(percent) => {
            crate::popup_size::PopupSize::Percent(percent)
        }
    }
}

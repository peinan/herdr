use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use super::{App, GIT_REMOTE_STATUS_REFRESH_INTERVAL, GIT_REPO_DISCOVERY_REFRESH_INTERVAL};
use crate::events::AppEvent;
use crate::ui::pane_title::{self, TitleField};
use crate::workspace::{GitStatusCacheEntry, GitStatusRefreshDemand, WorkspaceGitStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceGitRefreshItem {
    workspace_id: String,
    resolved_identity_cwd: PathBuf,
    cache_key_hint: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceGitRefreshTarget {
    workspace_id: String,
    resolved_identity_cwd: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceGitRefreshJob {
    cache_key: PathBuf,
    cached: Option<GitStatusCacheEntry>,
    targets: Vec<WorkspaceGitRefreshTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkspaceGitRefreshOutput {
    results: Vec<WorkspaceGitStatus>,
    cache_updates: Vec<(PathBuf, GitStatusCacheEntry)>,
}

impl App {
    pub(crate) fn start_git_status_refresh_if_due(&mut self, now: Instant) {
        let Some(deadline) = self.git_refresh_deadline() else {
            return;
        };

        if now < deadline {
            return;
        }

        // Advance each pane's cached cwd from its live runtime cwd before
        // collecting git targets. `terminal.cwd` is otherwise only moved by OSC 7
        // reports, so a shell that emits none would pin the pane title (and its
        // per-repo git lookup) to the spawn cwd. `cwd_for_pane` resolves the
        // runtime cwd (report, else a process-cwd syscall), keeping the title on
        // the same repo the refresh below computes.
        if !self.state.pane_title_format.is_empty() && self.sync_pane_cwds() {
            self.render_dirty.request_generic();
            self.render_notify.notify_one();
        }
        if !self.git_identity_refresh_requested && self.git_refresh_demand().is_empty() {
            // Only the pane title's cwd tracking was due; nothing needs git.
            self.last_git_remote_status_refresh = now;
            return;
        }

        let refresh_repo_discovery = self.git_identity_refresh_requested
            || now.saturating_duration_since(self.last_git_repo_discovery_refresh)
                >= GIT_REPO_DISCOVERY_REFRESH_INTERVAL;
        let workspaces = self.workspace_git_refresh_items(refresh_repo_discovery);

        if workspaces.is_empty() {
            self.last_git_remote_status_refresh = now;
            self.git_identity_refresh_requested = false;
            return;
        }

        self.git_refresh_in_flight = true;
        let event_tx = self.event_tx.clone();
        let cache = self.git_status_cache.clone();
        let mut demand = self.git_refresh_demand();
        if self.git_identity_refresh_requested {
            demand.branch = true;
        }
        self.git_identity_refresh_requested = false;
        if refresh_repo_discovery {
            self.last_git_repo_discovery_refresh = now;
        }
        std::thread::spawn(move || {
            let output =
                refresh_workspace_git_statuses_with_cache_and_demand(workspaces, &cache, demand);
            let _ = event_tx.blocking_send(AppEvent::GitStatusRefreshed {
                results: output.results,
                cache_updates: output.cache_updates,
            });
        });
    }

    pub(crate) fn request_git_identity_refresh(&mut self, now: Instant) {
        self.git_identity_refresh_requested = true;
        self.mark_git_status_refresh_due(now);
    }

    pub(crate) fn mark_git_status_refresh_due(&mut self, now: Instant) {
        self.git_status_cache
            .retain(|_, entry| entry.fingerprint.is_some());
        if self.git_refresh_in_flight {
            self.git_refresh_due_after_in_flight = true;
            return;
        }
        self.last_git_remote_status_refresh = now
            .checked_sub(GIT_REMOTE_STATUS_REFRESH_INTERVAL)
            .unwrap_or(now);
        self.git_refresh_due_after_in_flight = false;
    }

    pub(crate) fn git_refresh_deadline(&self) -> Option<Instant> {
        (!self.git_refresh_in_flight
            && !self.state.workspaces.is_empty()
            && (self.git_identity_refresh_requested
                || !self.git_refresh_demand().is_empty()
                || !self.state.pane_title_format.is_empty()))
        .then_some(self.last_git_remote_status_refresh + GIT_REMOTE_STATUS_REFRESH_INTERVAL)
    }

    fn git_refresh_demand(&self) -> GitStatusRefreshDemand {
        let mut demand = GitStatusRefreshDemand::default();
        for token in self.state.sidebar_spaces.rows.iter().flatten() {
            match token.parts().0 {
                crate::config::SpaceSidebarToken::Branch => demand.branch = true,
                crate::config::SpaceSidebarToken::GitStatus => demand.ahead_behind = true,
                _ => {}
            }
        }
        let title = self.pane_title_git_demand();
        demand.branch |= title.branch;
        demand.ahead_behind |= title.ahead_behind;
        demand.working_tree |= title.working_tree;
        demand
    }

    /// Git facts the pane title (`[ui] pane_title_format`) needs.
    ///
    /// The title reads per-repo snapshots mirrored from the cache entries
    /// `handle_git_status_refreshed` receives; every fingerprinted refresh
    /// path produces one, so only the fields the format uses are demanded.
    fn pane_title_git_demand(&self) -> GitStatusRefreshDemand {
        let title = &self.state.pane_title_format;
        let branch = pane_title::references(title, TitleField::Branch);
        let ahead_behind = pane_title::references(title, TitleField::AheadBehind);
        let working_tree = pane_title::references(title, TitleField::GitStatus);
        GitStatusRefreshDemand {
            branch,
            ahead_behind,
            working_tree,
        }
    }

    /// Refresh each pane's cached `terminal.cwd` from its live runtime cwd.
    ///
    /// The pane title and its per-repo git lookup both read `terminal.cwd`, which
    /// is otherwise advanced only by OSC 7 cwd reports. Shells that emit no OSC 7
    /// leave it pinned at the spawn cwd; resolving `cwd_for_pane` (runtime report,
    /// else a `proc_pidinfo`/`/proc` process-cwd syscall) keeps `$dir`/`$cwd` and
    /// the git status marks tracking the real cwd. Returns whether any changed.
    fn sync_pane_cwds(&mut self) -> bool {
        let mut updates: Vec<(crate::terminal::TerminalId, PathBuf)> = Vec::new();
        for ws in &self.state.workspaces {
            for tab in &ws.tabs {
                for &pane_id in tab.panes.keys() {
                    let Some(terminal_id) = tab.terminal_id(pane_id) else {
                        continue;
                    };
                    let Some(cwd) =
                        tab.cwd_for_pane(pane_id, &self.state.terminals, &self.terminal_runtimes)
                    else {
                        continue;
                    };
                    if !cwd.is_absolute() {
                        continue;
                    }
                    if self
                        .state
                        .terminals
                        .get(terminal_id)
                        .is_some_and(|terminal| terminal.cwd != cwd)
                    {
                        updates.push((terminal_id.clone(), cwd));
                    }
                }
            }
        }
        if updates.is_empty() {
            return false;
        }
        for (terminal_id, cwd) in updates {
            if let Some(terminal) = self.state.terminals.get_mut(&terminal_id) {
                terminal.cwd = cwd;
            }
        }
        self.state.mark_session_dirty();
        true
    }

    fn workspace_git_refresh_items(
        &self,
        refresh_repo_discovery: bool,
    ) -> Vec<WorkspaceGitRefreshItem> {
        let collect_pane_cwds = !self.pane_title_git_demand().is_empty();
        let mut items = Vec::new();
        for ws in &self.state.workspaces {
            // Workspace identity cwd drives the sidebar status (kept first so it
            // owns the deduplicated job for its repo key).
            if let Some(cwd) =
                ws.resolved_identity_cwd_from(&self.state.terminals, &self.terminal_runtimes)
            {
                let cache_key_hint = (!refresh_repo_discovery && ws.cached_identity_cwd == cwd)
                    .then(|| ws.cached_git_status_key.clone());
                items.push(WorkspaceGitRefreshItem {
                    workspace_id: ws.id.clone(),
                    resolved_identity_cwd: cwd,
                    cache_key_hint,
                });
            }
            if !collect_pane_cwds {
                continue;
            }

            // Each pane's resolved cwd so per-pane title rendering can resolve a
            // repo snapshot. Linked worktrees of the same repository get distinct
            // cache keys, so a pane in another worktree carries its own branch.
            // Deduplication folds panes that share a repo into one git call;
            // panes outside any repository are skipped.
            for tab in &ws.tabs {
                for &pane_id in tab.panes.keys() {
                    let Some(cwd) =
                        tab.cwd_for_pane(pane_id, &self.state.terminals, &self.terminal_runtimes)
                    else {
                        continue;
                    };
                    let Some(cache_key) = crate::workspace::git_status_cache_key(&cwd) else {
                        continue;
                    };
                    items.push(WorkspaceGitRefreshItem {
                        workspace_id: ws.id.clone(),
                        resolved_identity_cwd: cwd,
                        cache_key_hint: Some(cache_key),
                    });
                }
            }
        }
        items
    }
}

fn deduplicate_git_refresh_items(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
) -> Vec<WorkspaceGitRefreshJob> {
    let mut indexes = HashMap::<PathBuf, usize>::new();
    let mut jobs = Vec::<WorkspaceGitRefreshJob>::new();

    for item in items {
        let reconcile = item.cache_key_hint.is_none();
        let cache_key = item.cache_key_hint.unwrap_or_else(|| {
            crate::workspace::git_status_cache_key(&item.resolved_identity_cwd)
                .unwrap_or_else(|| item.resolved_identity_cwd.clone())
        });
        let target = WorkspaceGitRefreshTarget {
            workspace_id: item.workspace_id,
            resolved_identity_cwd: item.resolved_identity_cwd,
        };
        if let Some(&index) = indexes.get(&cache_key) {
            jobs[index].cached = jobs[index].cached.take().filter(|_| !reconcile);
            jobs[index].targets.push(target);
            continue;
        }

        let cached = cache.get(&cache_key).filter(|_| !reconcile).cloned();
        indexes.insert(cache_key.clone(), jobs.len());
        jobs.push(WorkspaceGitRefreshJob {
            cache_key,
            cached,
            targets: vec![target],
        });
    }

    jobs
}

fn refresh_workspace_git_statuses_with_cache_and_demand(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
    demand: GitStatusRefreshDemand,
) -> WorkspaceGitRefreshOutput {
    let mut results = Vec::new();
    let mut cache_updates = Vec::new();

    for job in deduplicate_git_refresh_items(items, cache) {
        let (snapshot, cache_entry) = crate::workspace::git_status_snapshot_for_cwd_with_demand(
            &job.cache_key,
            job.cached.as_ref(),
            demand,
        );
        if let Some(cache_entry) = cache_entry {
            cache_updates.push((job.cache_key.clone(), cache_entry));
        }
        results.extend(job.targets.into_iter().map(move |target| {
            snapshot.clone().into_workspace_status(
                target.workspace_id,
                target.resolved_identity_cwd,
                job.cache_key.clone(),
                demand,
            )
        }));
    }

    WorkspaceGitRefreshOutput {
        results,
        cache_updates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    #[test]
    fn git_refresh_deduplicates_workspaces_with_same_cache_key() {
        let repo =
            std::env::temp_dir().join(format!("herdr-git-refresh-dedupe-{}", std::process::id()));
        let nested = repo.join("nested");
        let other = repo.join("other");
        std::fs::create_dir_all(&nested).expect("create nested dir");
        std::fs::create_dir_all(&other).expect("create other dir");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .expect("run git init");

        let output = refresh_workspace_git_statuses_with_cache_and_demand(
            vec![
                WorkspaceGitRefreshItem {
                    workspace_id: "one".into(),
                    resolved_identity_cwd: nested.clone(),
                    cache_key_hint: None,
                },
                WorkspaceGitRefreshItem {
                    workspace_id: "two".into(),
                    resolved_identity_cwd: other.clone(),
                    cache_key_hint: None,
                },
            ],
            &HashMap::new(),
            GitStatusRefreshDemand::ALL,
        );

        assert_eq!(output.cache_updates.len(), 1);
        assert_eq!(
            output.cache_updates[0].0,
            std::fs::canonicalize(&repo).expect("canonical repo path")
        );
        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].workspace_id, "one");
        assert_eq!(output.results[0].resolved_identity_cwd, nested);
        assert_eq!(output.results[1].workspace_id, "two");
        assert_eq!(output.results[1].resolved_identity_cwd, other);

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn shared_root_repo_refresh_keeps_workspace_specific_fallback_labels() {
        let cache_key = PathBuf::from("/");
        let cached = GitStatusCacheEntry {
            fingerprint: None,
            retry_after: Some(Instant::now() + std::time::Duration::from_secs(30)),
            snapshot: crate::workspace::WorkspaceGitStatusSnapshot {
                auto_label: "/".into(),
                branch: Some("main".into()),
                ahead_behind: None,
                space: Some(crate::workspace::GitSpaceMetadata {
                    key: "/.git".into(),
                    checkout_key: "/".into(),
                    repo_name: "repo".into(),
                    repo_root: cache_key.clone(),
                    is_linked_worktree: false,
                }),
                working_tree: Default::default(),
                short_commit: None,
            },
        };
        let items = ["alpha", "beta"]
            .into_iter()
            .map(|name| WorkspaceGitRefreshItem {
                workspace_id: name.into(),
                resolved_identity_cwd: cache_key.join(name),
                cache_key_hint: Some(cache_key.clone()),
            })
            .collect();

        let output = refresh_workspace_git_statuses_with_cache_and_demand(
            items,
            &HashMap::from([(cache_key, cached)]),
            GitStatusRefreshDemand::ALL,
        );

        assert_eq!(output.cache_updates.len(), 1);
        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].auto_label, "alpha");
        assert_eq!(output.results[1].auto_label, "beta");
        assert_eq!(output.results[0].branch.as_deref(), Some("main"));
        assert_eq!(output.results[1].branch.as_deref(), Some("main"));
    }

    #[test]
    fn git_refresh_item_collection_does_not_discover_uncached_cwd() {
        let mut app = test_app(&crate::config::Config::default());
        let cwd = std::env::temp_dir().join(format!("herdr-uncached-cwd-{}", std::process::id()));
        let mut ws = Workspace::test_new("test");
        ws.identity_cwd = cwd.clone();
        ws.tabs.clear();
        app.state.workspaces.push(ws);

        let items = app.workspace_git_refresh_items(false);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].resolved_identity_cwd, cwd);
        assert_eq!(items[0].cache_key_hint, None);
    }

    #[test]
    fn git_refresh_item_collection_reuses_matching_cached_key() {
        let mut app = test_app(&crate::config::Config::default());
        let cwd = PathBuf::from("/repo/deep/nested");
        let cache_key = PathBuf::from("/repo");
        let mut ws = Workspace::test_new("test");
        ws.identity_cwd = cwd.clone();
        ws.cached_identity_cwd = cwd;
        ws.cached_git_status_key = cache_key.clone();
        ws.tabs.clear();
        app.state.workspaces.push(ws);

        let items = app.workspace_git_refresh_items(false);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].cache_key_hint, Some(cache_key));
    }

    #[test]
    fn periodic_repo_discovery_ignores_cached_key_hints() {
        let mut app = test_app(&crate::config::Config::default());
        let cwd = PathBuf::from("/repo/deep/nested");
        let mut ws = Workspace::test_new("test");
        ws.identity_cwd = cwd.clone();
        ws.cached_identity_cwd = cwd;
        ws.cached_git_status_key = PathBuf::from("/repo");
        ws.tabs.clear();
        app.state.workspaces.push(ws);

        let items = app.workspace_git_refresh_items(true);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].cache_key_hint, None);
        let cache_key = items[0].resolved_identity_cwd.clone();
        let cached = GitStatusCacheEntry {
            fingerprint: None,
            retry_after: None,
            snapshot: crate::workspace::WorkspaceGitStatusSnapshot {
                auto_label: "stale".into(),
                branch: None,
                ahead_behind: None,
                working_tree: Default::default(),
                short_commit: None,
                space: None,
            },
        };
        let jobs = deduplicate_git_refresh_items(items, &HashMap::from([(cache_key, cached)]));
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].cached, None);
    }

    #[test]
    fn cwd_identity_refresh_runs_once_without_sidebar_git_tokens() {
        let mut config = crate::config::Config::default();
        config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
        let mut app = test_app(&config);
        app.state.workspaces.push(Workspace::test_new("test"));
        let now = Instant::now();

        app.request_git_identity_refresh(now);

        assert!(app.git_refresh_deadline().is_some());
        app.start_git_status_refresh_if_due(now);
        assert!(app.git_refresh_in_flight);
        assert!(!app.git_identity_refresh_requested);
    }

    #[test]
    fn due_git_refresh_does_not_start_without_sidebar_consumer() {
        let mut config = crate::config::Config::default();
        config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
        let mut app = test_app(&config);
        app.state.workspaces.push(Workspace::test_new("test"));
        let now = Instant::now();
        app.last_git_remote_status_refresh = now - GIT_REMOTE_STATUS_REFRESH_INTERVAL;

        app.start_git_status_refresh_if_due(now);

        assert!(!app.git_refresh_in_flight);
        assert!(app.event_rx.try_recv().is_err());
    }

    #[test]
    fn git_refresh_demand_matches_sidebar_rows() {
        let cases = [
            (
                crate::config::SpaceSidebarToken::Workspace,
                GitStatusRefreshDemand::default(),
            ),
            (
                crate::config::SpaceSidebarToken::Branch,
                GitStatusRefreshDemand {
                    branch: true,
                    ahead_behind: false,
                    working_tree: false,
                },
            ),
            (
                crate::config::SpaceSidebarToken::GitStatus,
                GitStatusRefreshDemand {
                    branch: false,
                    ahead_behind: true,
                    working_tree: false,
                },
            ),
        ];

        for (token, expected) in cases {
            let mut config = crate::config::Config::default();
            config.ui.sidebar.spaces.rows = vec![vec![token.clone()]];
            let mut app = test_app(&config);
            app.state.workspaces.push(Workspace::test_new("test"));

            assert_eq!(app.git_refresh_demand(), expected, "token: {token:?}");
            assert_eq!(
                app.git_refresh_deadline().is_some(),
                !expected.is_empty(),
                "token: {token:?}"
            );
        }
    }

    #[test]
    fn unnamed_linked_worktree_does_not_force_periodic_branch_refresh() {
        let mut config = crate::config::Config::default();
        config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
        let mut app = test_app(&config);
        let mut child = Workspace::test_new("test");
        child.custom_name = None;
        child.worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo".into(),
            label: "repo".into(),
            repo_root: "/repo".into(),
            checkout_path: "/repo-worktree".into(),
            is_linked_worktree: true,
        });
        app.state.workspaces.push(child);

        assert_eq!(app.git_refresh_deadline(), None);
    }

    #[test]
    fn custom_named_linked_worktree_does_not_require_branch_refresh() {
        let mut config = crate::config::Config::default();
        config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
        let mut app = test_app(&config);
        let mut child = Workspace::test_new("custom");
        child.worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo".into(),
            label: "repo".into(),
            repo_root: "/repo".into(),
            checkout_path: "/repo-worktree".into(),
            is_linked_worktree: true,
        });
        app.state.workspaces.push(child);

        assert_eq!(app.git_refresh_deadline(), None);
    }

    #[test]
    fn headless_deadline_can_suppress_git_refresh_timer() {
        let mut app = test_app(&crate::config::Config::default());
        app.state.workspaces.push(Workspace::test_new("test"));
        let now = Instant::now();
        app.last_git_remote_status_refresh = now - GIT_REMOTE_STATUS_REFRESH_INTERVAL;

        assert_eq!(
            app.next_headless_loop_deadline_with_git_refresh(now, false, false),
            None
        );
        assert_eq!(
            app.next_headless_loop_deadline_with_git_refresh(now, false, true),
            Some(now)
        );
    }

    #[test]
    fn explicit_git_refresh_invalidates_cached_non_git_results() {
        let mut app = test_app(&crate::config::Config::default());
        let cwd = std::env::temp_dir().join(format!("herdr-git-miss-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).unwrap();
        let (_, entry) = crate::workspace::git_status_snapshot_for_cwd_with_demand(
            &cwd,
            None,
            GitStatusRefreshDemand::ALL,
        );
        app.git_status_cache
            .insert(cwd.clone(), entry.expect("non-Git cache entry"));

        app.mark_git_status_refresh_due(Instant::now());

        assert!(app.git_status_cache.is_empty());
        std::fs::remove_dir_all(cwd).unwrap();
    }

    #[test]
    fn git_refresh_due_request_survives_in_flight_refresh() {
        let mut app = test_app(&crate::config::Config::default());
        let now = Instant::now();
        app.git_refresh_in_flight = true;

        app.mark_git_status_refresh_due(now);
        assert!(app.git_refresh_due_after_in_flight);

        app.handle_internal_event(AppEvent::GitStatusRefreshed {
            results: Vec::new(),
            cache_updates: Vec::new(),
        });

        assert!(!app.git_refresh_in_flight);
        assert!(!app.git_refresh_due_after_in_flight);
        assert_eq!(app.git_refresh_deadline(), None);

        app.state.workspaces.push(Workspace::test_new("test"));
        let deadline = app
            .git_refresh_deadline()
            .expect("refresh should be due once a workspace exists");
        assert!(deadline <= Instant::now());
    }

    #[test]
    fn git_refresh_demand_includes_pane_title_fields() {
        let cases = [
            ("$dir $process", GitStatusRefreshDemand::default()),
            (
                "$branch",
                GitStatusRefreshDemand {
                    branch: true,
                    ahead_behind: false,
                    working_tree: false,
                },
            ),
            (
                "$dir( $git_status)",
                GitStatusRefreshDemand {
                    branch: false,
                    ahead_behind: false,
                    working_tree: true,
                },
            ),
        ];
        for (format, expected) in cases {
            let mut config = crate::config::Config::default();
            config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
            let mut app = test_app(&config);
            app.state.pane_title_format = pane_title::parse(format).expect("parse format");
            app.state.workspaces.push(Workspace::test_new("test"));

            assert_eq!(app.git_refresh_demand(), expected, "format: {format}");
            // A cwd-tracking title keeps the periodic tick alive even without git.
            assert!(app.git_refresh_deadline().is_some(), "format: {format}");
        }
    }

    #[test]
    fn cwd_only_pane_title_syncs_cwds_without_starting_git_refresh() {
        let mut config = crate::config::Config::default();
        config.ui.sidebar.spaces.rows = vec![vec![crate::config::SpaceSidebarToken::Workspace]];
        let mut app = test_app(&config);
        app.state.pane_title_format = pane_title::parse("$dir").expect("parse format");
        app.state.workspaces.push(Workspace::test_new("test"));
        app.state.ensure_test_terminals();
        let now = Instant::now();
        app.last_git_remote_status_refresh = now - GIT_REMOTE_STATUS_REFRESH_INTERVAL;

        app.start_git_status_refresh_if_due(now);

        assert!(!app.git_refresh_in_flight);
        assert!(app.event_rx.try_recv().is_err());
        assert_eq!(app.last_git_remote_status_refresh, now);
    }

    #[test]
    fn git_refresh_items_collect_pane_cwd_repo() {
        let base = std::env::temp_dir().join(format!(
            "herdr-pane-cwd-refresh-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // identity_cwd points at a directory that is not a git repo, so any
        // collected repo key must come from resolving the pane's cwd rather
        // than the identity fallback.
        let identity = base.join("plain");
        let repo = base.join("repo");
        std::fs::create_dir_all(&identity).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .unwrap();

        let mut app = test_app(&crate::config::Config::default());
        app.state.pane_title_format = pane_title::parse("$branch").expect("parse format");
        let mut ws = Workspace::test_new("test");
        ws.identity_cwd = identity.clone();
        let pane_id = ws.tabs[0].root_pane;
        let terminal_id = ws.tabs[0].panes[&pane_id].attached_terminal_id.clone();
        app.state.workspaces.push(ws);
        app.state.ensure_test_terminals();
        // Resolve the pane's shell cwd into the git repository.
        app.state.terminals.get_mut(&terminal_id).unwrap().cwd = repo.clone();

        let items = app.workspace_git_refresh_items(false);
        let repo_key = crate::workspace::git_status_cache_key(&repo).unwrap();

        assert!(
            items
                .iter()
                .any(|item| item.cache_key_hint.as_ref() == Some(&repo_key)),
            "pane cwd repo should be collected: {items:?}"
        );

        // Without a git field in the title, panes are not collected.
        app.state.pane_title_format = pane_title::parse("$dir").expect("parse format");
        let items = app.workspace_git_refresh_items(false);
        assert!(
            items
                .iter()
                .all(|item| item.cache_key_hint.as_ref() != Some(&repo_key)),
            "pane cwd repo should not be collected: {items:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn git_status_refresh_requests_render_on_working_tree_change() {
        let repo = std::env::temp_dir().join(format!(
            "herdr-git-render-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&repo).unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .expect("git init");

        let (_snapshot, entry) = crate::workspace::git_status_snapshot_for_cwd_with_demand(
            &repo,
            None,
            GitStatusRefreshDemand::ALL,
        );
        let entry = entry.expect("repo produces a git status cache entry");
        let key = crate::workspace::git_status_cache_key(&repo).expect("repo cache key");

        let mut app = test_app(&crate::config::Config::default());

        // A new per-repo snapshot must request a render even though no workspace
        // matches `results` (empty) — the pane title reads this map directly.
        let _ = app.render_dirty.take();
        app.handle_internal_event(AppEvent::GitStatusRefreshed {
            results: Vec::new(),
            cache_updates: vec![(key.clone(), entry.clone())],
        });
        assert!(app.render_dirty.take().generic);

        // An identical snapshot is not a change: no render requested.
        app.handle_internal_event(AppEvent::GitStatusRefreshed {
            results: Vec::new(),
            cache_updates: vec![(key.clone(), entry.clone())],
        });
        assert!(!app.render_dirty.take().generic);

        // A working-tree-only change (dirty edit; branch/ahead-behind unchanged)
        // must still request a render.
        let mut dirty = entry;
        dirty.snapshot.working_tree.modified += 1;
        app.handle_internal_event(AppEvent::GitStatusRefreshed {
            results: Vec::new(),
            cache_updates: vec![(key, dirty)],
        });
        assert!(app.render_dirty.take().generic);

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn sync_pane_cwds_is_noop_when_runtime_cwd_matches_cached() {
        // With no runtime registered, `cwd_for_pane` falls back to the cached
        // `terminal.cwd`, so the sync must find nothing to change and must not
        // dirty the session (which would churn a render on every git tick).
        let mut app = test_app(&crate::config::Config::default());
        app.state.workspaces.push(Workspace::test_new("test"));
        app.state.ensure_test_terminals();
        app.state.session_dirty = false;

        assert!(!app.sync_pane_cwds());
        assert!(!app.state.session_dirty);
    }

    fn test_app(config: &crate::config::Config) -> super::super::App {
        super::super::App::new(
            config,
            true,
            None,
            tokio::sync::mpsc::unbounded_channel().1,
            crate::api::EventHub::default(),
        )
    }
}

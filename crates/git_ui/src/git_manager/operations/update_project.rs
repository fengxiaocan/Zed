use crate::git_manager_settings::GitManagerSettings;
use askpass::AskPassDelegate;
use git::repository::{FetchOptions, Remote, UpstreamTracking};
use gpui::{App, Entity, Window};
use project::git_store::Repository;
use settings::{Settings, translate_ui};
use util::ResultExt;
use workspace::Workspace;
use workspace::notifications::DetachAndPromptErr;

/// How Update Project should integrate fetched changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateMode {
    Rebase,
    Merge,
    OnlyFetch,
}

impl From<settings::UpdateProjectMode> for UpdateMode {
    fn from(mode: settings::UpdateProjectMode) -> Self {
        match mode {
            settings::UpdateProjectMode::Rebase => UpdateMode::Rebase,
            settings::UpdateProjectMode::Merge => UpdateMode::Merge,
            settings::UpdateProjectMode::OnlyFetch => UpdateMode::OnlyFetch,
        }
    }
}

/// What to do when the worktree has uncommitted changes before integrating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirtyPolicy {
    Ask,
    ShelveFirst,
    AlwaysContinue,
}

impl From<settings::UpdateProjectDirtyWorktree> for DirtyPolicy {
    fn from(policy: settings::UpdateProjectDirtyWorktree) -> Self {
        match policy {
            settings::UpdateProjectDirtyWorktree::Ask => DirtyPolicy::Ask,
            settings::UpdateProjectDirtyWorktree::ShelveFirst => DirtyPolicy::ShelveFirst,
            settings::UpdateProjectDirtyWorktree::AlwaysContinue => DirtyPolicy::AlwaysContinue,
        }
    }
}

/// Pure decision helper describing what Update Project should do (unit-tested).
#[derive(Debug, PartialEq)]
pub(crate) enum UpdateProjectPlan {
    /// Nothing to integrate; the caller still performs the fetch.
    FetchOnly,
    FetchAndRebase { upstream: String },
    FetchAndMerge { upstream: String },
    /// No upstream branch configured; cannot integrate.
    NeedUpstream,
    /// Worktree is dirty and policy requires asking; wraps the plan to run after.
    PromptDirtyWorktree { next: Box<UpdateProjectPlan> },
    /// Worktree is dirty and policy is shelve-first: stash, then run the plan.
    ShelveThenIntegrate { next: Box<UpdateProjectPlan> },
}

pub(crate) fn plan_update_project(
    mode: UpdateMode,
    upstream: Option<String>,
    behind: u64,
    dirty: bool,
    dirty_policy: DirtyPolicy,
) -> UpdateProjectPlan {
    let Some(upstream) = upstream else {
        return UpdateProjectPlan::NeedUpstream;
    };

    let integrate = if mode == UpdateMode::OnlyFetch || behind == 0 {
        UpdateProjectPlan::FetchOnly
    } else {
        match mode {
            UpdateMode::Rebase => UpdateProjectPlan::FetchAndRebase { upstream },
            UpdateMode::Merge => UpdateProjectPlan::FetchAndMerge { upstream },
            UpdateMode::OnlyFetch => unreachable!("handled above"),
        }
    };

    match (dirty, dirty_policy) {
        // OnlyFetch never touches the worktree, so no dirty handling is needed.
        _ if integrate == UpdateProjectPlan::FetchOnly => integrate,
        (true, DirtyPolicy::Ask) => UpdateProjectPlan::PromptDirtyWorktree {
            next: Box::new(integrate),
        },
        (true, DirtyPolicy::ShelveFirst) => UpdateProjectPlan::ShelveThenIntegrate {
            next: Box::new(integrate),
        },
        _ => integrate,
    }
}

/// A snapshot of the inputs needed to plan Update Project, taken synchronously
/// from the repository before any async work begins.
struct UpdateInputs {
    plan: UpdateProjectPlan,
    remote: Remote,
}

/// Reads the current branch, its upstream, behind-count and dirty state, and
/// resolves which remote to fetch from.
fn plan_inputs(
    repo: &Entity<Repository>,
    project_is_local: bool,
    cx: &App,
) -> Option<UpdateInputs> {
    if !project_is_local {
        return None;
    }
    let snapshot = repo.read(cx);
    let branch = snapshot.branch.as_ref()?;
    let upstream = branch.upstream.as_ref()?;
    if !upstream.is_remote() {
        return None;
    }
    let behind = match upstream.tracking {
        UpstreamTracking::Tracked(status) => status.behind as u64,
        UpstreamTracking::Gone => 0,
    };
    let remote_name = upstream.remote_name()?.to_string();
    // We only need the remote's name to fetch; there is no snapshot list of
    // remotes, so build the handle directly from the upstream's remote name.
    let remote = Remote {
        name: remote_name.into(),
    };

    let settings = GitManagerSettings::get_global(cx);
    let dirty = snapshot.status_summary().count > 0;
    let plan = plan_update_project(
        settings.update_project_mode.into(),
        Some(upstream.stripped_ref_name()?.to_string()),
        behind,
        dirty,
        settings.update_project_dirty_worktree.into(),
    );
    Some(UpdateInputs { plan, remote })
}

fn askpass_delegate(
    workspace: &gpui::WeakEntity<Workspace>,
    operation: impl Into<gpui::SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> AskPassDelegate {
    let workspace = workspace.clone();
    let operation = operation.into();
    let window_handle = window.window_handle();
    AskPassDelegate::new(&mut window.to_async(cx), move |prompt, tx, cx| {
        window_handle
            .update(cx, |_, window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace.toggle_modal(window, cx, |window, cx| {
                        crate::askpass_modal::AskPassModal::new(
                            operation.clone(),
                            prompt.into(),
                            tx,
                            window,
                            cx,
                        )
                    });
                })
            })
            .log_err();
    })
}

/// Runs Update Project for the active repository: fetch, then rebase/merge per
/// the configured mode, honoring the dirty-worktree policy.
pub(crate) fn update_project(
    repo: &Entity<Repository>,
    workspace: &gpui::WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let project_is_local = workspace
        .upgrade()
        .map(|ws| ws.read(cx).project().read(cx).is_local())
        .unwrap_or(false);

    let Some(inputs) = plan_inputs(repo, project_is_local, cx) else {
        // Surface the "no upstream" case via the prompt error path below.
        window
            .spawn(cx, async move |_| -> anyhow::Result<()> {
                anyhow::bail!("The current branch has no upstream to update from.")
            })
            .detach_and_prompt_err(
                translate_ui("Update Project failed", cx),
                window,
                cx,
                |e, _, _| Some(e.to_string()),
            );
        return;
    };

    let dirty_label = translate_ui("Continue", cx);
    let stash_label = translate_ui("Shelve & Continue", cx);
    let cancel_label = translate_ui("Cancel", cx);

    let plan = inputs.plan;
    let repo = repo.clone();
    let workspace = workspace.clone();

    window
        .spawn(cx, async move |cx| {
            // Resolve the plan, applying the dirty-worktree policy.
            let plan = match plan {
                UpdateProjectPlan::PromptDirtyWorktree { next } => {
                    let prompt_window = cx.window_handle();
                    let answer = prompt_window
                        .update(cx, |_, window, cx| {
                            window.prompt(
                                gpui::PromptLevel::Warning,
                                translate_ui("You have uncommitted changes", cx),
                                Some(translate_ui(
                                    "Update Project will integrate fetched changes. Continue, or shelve (stash) your changes first?",
                                    cx,
                                )),
                                &[dirty_label, stash_label, cancel_label],
                                cx,
                            )
                        })?
                        .await
                        .map_err(|_| anyhow::anyhow!("Update Project cancelled"))?;

                    match answer {
                        // Continue
                        0 => *next,
                        // Shelve & Continue
                        1 => {
                            let task = repo.update(cx, |repo, cx| repo.stash_all(cx));
                            task.await?;
                            *next
                        }
                        _ => return Ok(()),
                    }
                }
                UpdateProjectPlan::ShelveThenIntegrate { next } => {
                    let task = repo.update(cx, |repo, cx| repo.stash_all(cx));
                    task.await?;
                    *next
                }
                plan => plan,
            };

            // Fetch from the resolved remote.
            let askpass = cx.window_handle().update(cx, |_, window, cx| {
                askpass_delegate(
                    &workspace,
                    format!("git fetch {}", inputs.remote.name),
                    window,
                    cx,
                )
            })?;
            let fetch = repo.update(cx, |repo, cx| {
                repo.fetch(FetchOptions::Remote(inputs.remote.clone()), askpass, cx)
            });
            fetch.await??;

            // Integrate per mode.
            match plan {
                UpdateProjectPlan::FetchOnly => {}
                UpdateProjectPlan::FetchAndRebase { upstream } => {
                    repo.update(cx, |repo, _| repo.rebase(upstream)).await??;
                }
                UpdateProjectPlan::FetchAndMerge { upstream } => {
                    repo.update(cx, |repo, _| repo.merge(upstream)).await??;
                }
                UpdateProjectPlan::NeedUpstream => {
                    anyhow::bail!("The current branch has no upstream to update from.")
                }
                UpdateProjectPlan::PromptDirtyWorktree { .. }
                | UpdateProjectPlan::ShelveThenIntegrate { .. } => {
                    unreachable!("dirty handling resolved above")
                }
            }

            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Update Project failed", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: &str = "origin/main";

    fn plan(
        mode: UpdateMode,
        upstream: Option<&str>,
        behind: u64,
        dirty: bool,
        policy: DirtyPolicy,
    ) -> UpdateProjectPlan {
        plan_update_project(mode, upstream.map(str::to_string), behind, dirty, policy)
    }

    #[test]
    fn no_upstream_needs_upstream() {
        assert_eq!(
            plan(UpdateMode::Rebase, None, 3, false, DirtyPolicy::Ask),
            UpdateProjectPlan::NeedUpstream
        );
        // Even with fetch-only mode, no upstream means we cannot fetch from it.
        assert_eq!(
            plan(UpdateMode::OnlyFetch, None, 0, false, DirtyPolicy::Ask),
            UpdateProjectPlan::NeedUpstream
        );
    }

    #[test]
    fn only_fetch_ignores_behind_and_dirty() {
        assert_eq!(
            plan(UpdateMode::OnlyFetch, Some(UP), 5, true, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchOnly
        );
    }

    #[test]
    fn up_to_date_is_fetch_only() {
        for mode in [UpdateMode::Rebase, UpdateMode::Merge] {
            assert_eq!(
                plan(mode, Some(UP), 0, false, DirtyPolicy::Ask),
                UpdateProjectPlan::FetchOnly
            );
        }
    }

    #[test]
    fn behind_rebases_or_merges() {
        assert_eq!(
            plan(UpdateMode::Rebase, Some(UP), 2, false, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchAndRebase {
                upstream: UP.into()
            }
        );
        assert_eq!(
            plan(UpdateMode::Merge, Some(UP), 2, false, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchAndMerge {
                upstream: UP.into()
            }
        );
    }

    #[test]
    fn dirty_ask_prompts_but_wraps_integrate() {
        assert_eq!(
            plan(UpdateMode::Rebase, Some(UP), 2, true, DirtyPolicy::Ask),
            UpdateProjectPlan::PromptDirtyWorktree {
                next: Box::new(UpdateProjectPlan::FetchAndRebase {
                    upstream: UP.into()
                })
            }
        );
        // Fetch-only is never wrapped in a dirty prompt.
        assert_eq!(
            plan(UpdateMode::OnlyFetch, Some(UP), 2, true, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchOnly
        );
        assert_eq!(
            plan(UpdateMode::Rebase, Some(UP), 0, true, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchOnly
        );
    }

    #[test]
    fn dirty_shelve_first_shelves_then_integrates() {
        assert_eq!(
            plan(UpdateMode::Merge, Some(UP), 1, true, DirtyPolicy::ShelveFirst),
            UpdateProjectPlan::ShelveThenIntegrate {
                next: Box::new(UpdateProjectPlan::FetchAndMerge {
                    upstream: UP.into()
                })
            }
        );
        // Shelve-first on a fetch-only update is a no-op (nothing to integrate).
        assert_eq!(
            plan(UpdateMode::OnlyFetch, Some(UP), 1, true, DirtyPolicy::ShelveFirst),
            UpdateProjectPlan::FetchOnly
        );
    }

    #[test]
    fn dirty_always_continue_integrates_without_shelving() {
        assert_eq!(
            plan(
                UpdateMode::Rebase,
                Some(UP),
                1,
                true,
                DirtyPolicy::AlwaysContinue
            ),
            UpdateProjectPlan::FetchAndRebase {
                upstream: UP.into()
            }
        );
    }

    #[test]
    fn clean_never_prompts() {
        assert_eq!(
            plan(UpdateMode::Rebase, Some(UP), 2, false, DirtyPolicy::Ask),
            UpdateProjectPlan::FetchAndRebase {
                upstream: UP.into()
            }
        );
    }
}

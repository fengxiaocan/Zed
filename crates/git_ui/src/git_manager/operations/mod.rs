mod update_project;

pub(crate) use update_project::{askpass_delegate, update_project};

use gpui::{App, AsyncApp, PromptLevel, Window};
use project::git_store::Repository;
use settings::translate_ui;
use util::ResultExt;
use workspace::notifications::DetachAndPromptErr;

/// Merges `rev` into the current branch.
pub(crate) fn merge_branch(
    repo: &gpui::Entity<Repository>,
    rev: String,
    workspace: Option<gpui::WeakEntity<workspace::Workspace>>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    let current_branch = repo
        .read(cx)
        .branch
        .as_ref()
        .map(|b| b.name().to_string())
        .unwrap_or_else(|| "HEAD".to_string());
    let prompt_title = format!(
        "{} '{rev}' {} '{current_branch}'?",
        translate_ui("Merge branch", cx),
        translate_ui("into", cx)
    );
    let prompt_detail = format!(
        "{} '{rev}' {} '{current_branch}'?",
        translate_ui("Are you sure you want to merge", cx),
        translate_ui("into", cx)
    );
    let buttons = [translate_ui("Merge", cx), translate_ui("Cancel", cx)];
    let answer = window.prompt(
        PromptLevel::Info,
        &prompt_title,
        Some(&prompt_detail),
        &buttons,
        cx,
    );

    window
        .spawn(cx, async move |cx| {
            if answer.await != Ok(0) {
                return anyhow::Ok(());
            }

            let merge_res = repo.update(cx, |repo, _| repo.merge(rev)).await?;
            let is_conflict = if merge_res.is_err() {
                let check_rx = repo.update(cx, |repo, _| repo.is_merge_in_progress());
                check_rx
                    .await
                    .log_err()
                    .and_then(|r| r.log_err())
                    .unwrap_or(false)
            } else {
                false
            };

            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }

            if is_conflict {
                if let Some(workspace) = workspace {
                    workspace.update_in(cx, |workspace, window, cx| {
                        crate::conflict_modal::GitConflictModal::open(
                            workspace,
                            repo,
                            false,
                            window,
                            cx,
                        );
                    })?;
                }
                return anyhow::Ok(());
            }

            merge_res?;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Merge failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Rebases the current branch onto `onto`.
pub(crate) fn rebase_branch(
    repo: &gpui::Entity<Repository>,
    onto: String,
    workspace: Option<gpui::WeakEntity<workspace::Workspace>>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    let current_branch = repo
        .read(cx)
        .branch
        .as_ref()
        .map(|b| b.name().to_string())
        .unwrap_or_else(|| "HEAD".to_string());
    let prompt_title = format!(
        "{} '{current_branch}' {} '{onto}'?",
        translate_ui("Rebase branch", cx),
        translate_ui("onto", cx)
    );
    let prompt_detail = translate_ui(
        "This will replay commits from your current branch on top of the target branch.",
        cx,
    );
    let buttons = [translate_ui("Rebase", cx), translate_ui("Cancel", cx)];
    let answer = window.prompt(
        PromptLevel::Info,
        &prompt_title,
        Some(prompt_detail),
        &buttons,
        cx,
    );

    window
        .spawn(cx, async move |cx| {
            if answer.await != Ok(0) {
                return anyhow::Ok(());
            }

            let rebase_res = repo.update(cx, |repo, _| repo.rebase(onto)).await?;
            let is_conflict = if rebase_res.is_err() {
                let check_rx = repo.update(cx, |repo, _| repo.is_rebase_in_progress());
                check_rx
                    .await
                    .log_err()
                    .and_then(|r| r.log_err())
                    .unwrap_or(false)
            } else {
                false
            };

            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }

            if is_conflict {
                if let Some(workspace) = workspace {
                    workspace.update_in(cx, |workspace, window, cx| {
                        crate::conflict_modal::GitConflictModal::open(
                            workspace,
                            repo,
                            true,
                            window,
                            cx,
                        );
                    })?;
                }
                return anyhow::Ok(());
            }

            rebase_res?;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Rebase failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Aborts an in-progress merge.
pub(crate) fn merge_abort(
    repo: &gpui::Entity<Repository>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.merge_abort()).await??;
            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Abort merge failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Continues an in-progress rebase.
pub(crate) fn rebase_continue(
    repo: &gpui::Entity<Repository>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase_continue())
                .await??;
            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Rebase continue failed", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

/// Skips the current patch in an in-progress rebase.
pub(crate) fn rebase_skip(
    repo: &gpui::Entity<Repository>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase_skip()).await??;
            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Rebase skip failed", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

/// Aborts an in-progress rebase.
pub(crate) fn rebase_abort(
    repo: &gpui::Entity<Repository>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase_abort()).await??;
            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_all(cx);
                });
            }
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Abort rebase failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Query merge/rebase in-progress state; returns (merge_in_progress, rebase_in_progress).
pub(crate) async fn query_in_progress(
    repo: &gpui::Entity<Repository>,
    cx: &mut AsyncApp,
) -> (bool, bool) {
    let merge_rx = repo.update(cx, |repo, _| repo.is_merge_in_progress());
    let rebase_rx = repo.update(cx, |repo, _| repo.is_rebase_in_progress());
    let merge = async move {
        merge_rx
            .await
            .log_err()
            .and_then(|r| r.log_err())
            .unwrap_or(false)
    };
    let rebase = async move {
        rebase_rx
            .await
            .log_err()
            .and_then(|r| r.log_err())
            .unwrap_or(false)
    };
    futures::future::join(merge, rebase).await
}

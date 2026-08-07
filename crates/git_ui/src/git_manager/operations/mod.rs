mod update_project;

pub(crate) use update_project::update_project;

use gpui::{App, AsyncApp, Window};
use project::git_store::Repository;
use settings::translate_ui;
use util::ResultExt;
use workspace::notifications::DetachAndPromptErr;

/// Merges `rev` into the current branch.
pub(crate) fn merge_branch(
    repo: &gpui::Entity<Repository>,
    rev: String,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.merge(rev)).await??;
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
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase(onto)).await??;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Rebase failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Aborts an in-progress merge.
pub(crate) fn merge_abort(repo: &gpui::Entity<Repository>, window: &mut Window, cx: &mut App) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.merge_abort()).await??;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(translate_ui("Abort merge failed", cx), window, cx, |e, _, _| {
            Some(e.to_string())
        });
}

/// Continues an in-progress rebase.
pub(crate) fn rebase_continue(repo: &gpui::Entity<Repository>, window: &mut Window, cx: &mut App) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase_continue())
                .await??;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Rebase continue failed", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

/// Aborts an in-progress rebase.
pub(crate) fn rebase_abort(repo: &gpui::Entity<Repository>, window: &mut Window, cx: &mut App) {
    let repo = repo.clone();
    window
        .spawn(cx, async move |cx| {
            repo.update(cx, |repo, _| repo.rebase_abort()).await??;
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

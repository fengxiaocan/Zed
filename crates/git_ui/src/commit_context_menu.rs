use crate::commit_view::CommitView;
use git::Oid;
use gpui::{Action, ClipboardItem, Entity, FocusHandle, SharedString, WeakEntity, Window, actions};
use project::{GIT_COMMAND_TASK_TAG, git_store::Repository};
use settings::translate_ui;

use task::{TaskContext, TaskVariables, VariableName};
use ui::{Color, ContextMenu, ContextMenuEntry, IconName, IconPosition, prelude::*};
use workspace::Workspace;
use workspace::notifications::DetachAndPromptErr;

actions!(
    git_graph,
    [
        /// Copies the SHA of the selected commit to the clipboard.
        CopyCommitSha,
        /// Copies a tag from the selected commit to the clipboard.
        CopyCommitTag,
        /// Opens the commit view for the selected commit.
        OpenCommitView,
    ]
);

const COMMIT_TAG_LIST_WIDTH_IN_REMS: Rems = rems(10.);
const CUSTOM_GIT_COMMANDS_DOCS_SLUG: &str = "tasks#custom-git-commands";

pub(crate) struct CommitContextMenuData {
    pub(crate) sha: Oid,
    pub(crate) tag_names: Vec<SharedString>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitContextMenuSource {
    GitGraph,
    GitPanel,
}

pub(crate) fn commit_context_menu(
    commit: CommitContextMenuData,
    source: CommitContextMenuSource,
    ref_name: Option<SharedString>,
    focus_handle: FocusHandle,
    repository: Option<WeakEntity<Repository>>,
    workspace: WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ContextMenu> {
    let sha = commit.sha;
    let sha_short = sha.display_short();
    let git_tasks = git_context_menu_tasks(
        git_task_context(&repository, sha, ref_name.as_deref(), cx),
        &workspace,
        cx,
    );
    let header = match &ref_name {
        Some(ref_name) => format!("{} {ref_name}", translate_ui("Ref", cx)),
        None => format!("{} {sha_short}", translate_ui("Commit", cx)),
    };
    let view_diff_label = translate_ui("View Diff", cx);
    let checkout_commit_label = translate_ui("Checkout Commit…", cx);
    let new_branch_label = translate_ui("New Branch from Commit…", cx);
    let new_tag_label = translate_ui("New Tag on Commit…", cx);
    let reset_branch_label = translate_ui("Reset Current Branch to Here…", cx);
    let copy_sha_label = translate_ui("Copy SHA", cx);
    let copy_ref_name_label = translate_ui("Copy Ref Name", cx);
    let copy_tag_label = translate_ui("Copy Tag", cx);
    let show_in_git_graph_label = translate_ui("Show in Git Graph", cx);
    let custom_commands_label = translate_ui("Custom Commands", cx);
    let learn_more_label = translate_ui("Learn More", cx);

    ContextMenu::build(window, cx, move |context_menu, _, _| {
        context_menu
            .context(focus_handle)
            .header(header)
            .entry(view_diff_label, Some(OpenCommitView.boxed_clone()), {
                let repository = repository.clone();
                let workspace = workspace.clone();
                move |window, cx| {
                    let Some(repository) = repository.clone() else {
                        return;
                    };
                    CommitView::open(
                        sha.to_string(),
                        repository,
                        workspace.clone(),
                        None,
                        None,
                        window,
                        cx,
                    );
                }
            })
            .entry(checkout_commit_label, None, {
                let repository = repository.clone();
                let sha_str = sha.to_string();
                let sha_short = sha_short.clone();
                move |window, cx| {
                    let Some(repo) = repository.clone().and_then(|r| r.upgrade()) else {
                        return;
                    };
                    let sha_str = sha_str.clone();
                    let prompt_title = format!("{} ({})", translate_ui("Checkout Commit", cx), sha_short);
                    let prompt_detail = translate_ui(
                        "Checking out this commit will put the repository into a 'detached HEAD' state. You can inspect changes or make experimental commits.",
                        cx,
                    );
                    let buttons = [translate_ui("Checkout", cx), translate_ui("Cancel", cx)];
                    let answer = window.prompt(
                        gpui::PromptLevel::Info,
                        &prompt_title,
                        Some(&prompt_detail),
                        &buttons,
                        cx,
                    );
                    window
                        .spawn(cx, async move |cx| {
                            if answer.await == Ok(0) {
                                let receiver = repo.update(cx, |repo, _| repo.change_branch(sha_str));
                                receiver.await??;
                            }
                            anyhow::Ok(())
                        })
                        .detach_and_prompt_err(
                            translate_ui("Failed to checkout commit", cx),
                            window,
                            cx,
                            |e, _, _| Some(e.to_string()),
                        );
                }
            })
            .entry(new_branch_label, None, {
                let repository = repository.clone();
                let workspace = workspace.clone();
                let sha_str = sha.to_string();
                let sha_short = sha_short.clone();
                move |window, cx| {
                    let Some(repo) = repository.clone().and_then(|r| r.upgrade()) else {
                        return;
                    };
                    let sha_str = sha_str.clone();
                    let sha_short = sha_short.clone();
                    if let Some(workspace) = workspace.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            let title = format!(
                                "{} ({})",
                                translate_ui("New Branch from Commit", cx),
                                sha_short
                            );
                            workspace.toggle_modal(window, cx, |window, cx| {
                                crate::NewBranchModal::new(
                                    title,
                                    String::new(),
                                    Some(sha_str),
                                    repo,
                                    window,
                                    cx,
                                )
                            });
                        });
                    }
                }
            })
            .entry(new_tag_label, None, {
                let repository = repository.clone();
                let workspace = workspace.clone();
                let sha_str = sha.to_string();
                move |window, cx| {
                    let Some(repo) = repository.clone().and_then(|r| r.upgrade()) else {
                        return;
                    };
                    let sha_str = sha_str.clone();
                    if let Some(workspace) = workspace.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.toggle_modal(window, cx, |window, cx| {
                                crate::git_manager::sections::NewTagModal::new(
                                    repo,
                                    Some(sha_str),
                                    None,
                                    window,
                                    cx,
                                )
                            });
                        });
                    }
                }
            })
            .entry(reset_branch_label, None, {
                let repository = repository.clone();
                let sha_str = sha.to_string();
                let sha_short = sha_short.clone();
                move |window, cx| {
                    let Some(repo) = repository.clone().and_then(|r| r.upgrade()) else {
                        return;
                    };
                    let sha_str = sha_str.clone();
                    let sha_short = sha_short.clone();
                    let prompt_title = format!(
                        "{} ({})",
                        translate_ui("Reset Current Branch to Commit", cx),
                        sha_short
                    );
                    let prompt_detail = translate_ui(
                        "Choose reset mode:\n• Mixed: Keep working tree changes (unstaged)\n• Soft: Keep working tree and index changes (staged)",
                        cx,
                    );
                    let buttons = [
                        translate_ui("Mixed (Keep Unstaged)", cx),
                        translate_ui("Soft (Keep Staged)", cx),
                        translate_ui("Cancel", cx),
                    ];
                    let answer = window.prompt(
                        gpui::PromptLevel::Warning,
                        &prompt_title,
                        Some(&prompt_detail),
                        &buttons,
                        cx,
                    );
                    window
                        .spawn(cx, async move |cx| {
                            let mode = match answer.await {
                                Ok(0) => git::repository::ResetMode::Mixed,
                                Ok(1) => git::repository::ResetMode::Soft,
                                _ => return anyhow::Ok(()),
                            };
                            let receiver = repo.update(cx, |repo, cx| repo.reset(sha_str, mode, cx));
                            receiver.await??;
                            anyhow::Ok(())
                        })
                        .detach_and_prompt_err(
                            translate_ui("Failed to reset branch", cx),
                            window,
                            cx,
                            |e, _, _| Some(e.to_string()),
                        );
                }
            })
            .separator()
            .entry(
                copy_sha_label,
                Some(CopyCommitSha.boxed_clone()),
                move |_window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(sha.to_string()));
                },
            )
            .when_some(ref_name.clone(), |menu, ref_name| {
                menu.entry(copy_ref_name_label, None, move |_window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(ref_name.to_string()));
                })
            })
            .when(ref_name.is_none(), |menu| {
                menu.map(|menu| {
                    let tag_names = commit.tag_names.clone();

                    match tag_names.as_slice() {
                        [] => menu.item(
                            ContextMenuEntry::new(copy_tag_label)
                                .action(CopyCommitTag.boxed_clone())
                                .disabled(true),
                        ),
                        [tag_name] => {
                            let tag_name = tag_name.clone();
                            let label = format!("{copy_tag_label}: {tag_name}");
                            menu.entry(
                                label,
                                Some(CopyCommitTag.boxed_clone()),
                                move |_window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        tag_name.to_string(),
                                    ));
                                },
                            )
                        }
                        _ => menu.submenu(copy_tag_label, move |menu, _window, _cx| {
                            let mut menu = menu.fixed_width(COMMIT_TAG_LIST_WIDTH_IN_REMS.into());

                            for tag_name in tag_names.clone() {
                                let tag_name_to_copy = tag_name.clone();
                                menu = menu.entry(tag_name, None, move |_window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        tag_name_to_copy.to_string(),
                                    ));
                                });
                            }
                            menu
                        }),
                    }
                })
            })
            .when(source == CommitContextMenuSource::GitPanel, |menu| {
                menu.entry(show_in_git_graph_label, None, move |window, cx| {
                    window.dispatch_action(
                        Box::new(crate::git_graph::OpenAtCommit {
                            sha: sha.to_string(),
                        }),
                        cx,
                    );
                })
            })
            .map(|mut menu| {
                menu = menu.separator().header(custom_commands_label);

                if git_tasks.is_empty() {
                    return menu.item(
                        ContextMenuEntry::new(learn_more_label)
                            .icon(IconName::ArrowUpRight)
                            .icon_color(Color::Muted)
                            .icon_position(IconPosition::End)
                            .handler(|_window, cx| {
                                let docs_url =
                                    release_channel::docs_url(CUSTOM_GIT_COMMANDS_DOCS_SLUG, cx);
                                cx.open_url(&docs_url);
                            }),
                    );
                }

                for (task_source_kind, resolved_task) in git_tasks {
                    let label = resolved_task.display_label().to_string();
                    let workspace = workspace.clone();
                    menu = menu.entry(label, None, move |window, cx| {
                        workspace
                            .update(cx, |workspace, cx| {
                                workspace.schedule_resolved_task(
                                    task_source_kind.clone(),
                                    resolved_task.clone(),
                                    false,
                                    window,
                                    cx,
                                );
                            })
                            .ok();
                    });
                }

                menu
            })
    })
}

fn git_task_context(
    repository: &Option<WeakEntity<Repository>>,
    commit_sha: git::Oid,
    ref_name: Option<&str>,
    cx: &App,
) -> Option<TaskContext> {
    let repository_path = repository
        .as_ref()?
        .upgrade()?
        .read(cx)
        .work_directory_abs_path
        .to_path_buf();
    let repository_name = repository_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string);
    let mut task_variables = TaskVariables::from_iter([
        (VariableName::GitSha, commit_sha.to_string()),
        (VariableName::GitShaShort, commit_sha.display_short()),
        (
            VariableName::GitRepositoryPath,
            repository_path.to_string_lossy().into_owned(),
        ),
    ]);

    if let Some(repository_name) = repository_name {
        task_variables.insert(VariableName::GitRepositoryName, repository_name);
    }
    if let Some(ref_name) = ref_name {
        task_variables.insert(VariableName::GitRef, ref_name.to_string());
    }

    Some(TaskContext {
        cwd: Some(repository_path),
        task_variables,
        ..TaskContext::default()
    })
}

fn git_context_menu_tasks(
    task_context: Option<TaskContext>,
    workspace: &WeakEntity<Workspace>,
    cx: &App,
) -> Vec<(project::TaskSourceKind, task::ResolvedTask)> {
    let Some(task_context) = task_context else {
        return Vec::new();
    };
    let Some(workspace) = workspace.upgrade() else {
        return Vec::new();
    };
    let project = workspace.read(cx).project().clone();
    let task_inventory = project.read_with(cx, |project, cx| {
        project.task_store().read(cx).task_inventory().cloned()
    });
    let Some(task_inventory) = task_inventory else {
        return Vec::new();
    };

    task_inventory
        .read(cx)
        .resolve_global_tasks_with_tag(GIT_COMMAND_TASK_TAG, &task_context)
}

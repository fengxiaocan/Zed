use editor::{Editor, EditorEvent};
use git::repository::Branch;
use gpui::{
    App, ClipboardItem, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, PromptLevel, SharedString, Styled, Window, div, uniform_list,
};
use project::git_store::Repository;
use settings::translate_ui;
use ui::{
    Color, ContextMenu, IconButton, IconName, IconSize, Label, LabelSize, PopoverMenu, Tooltip,
    prelude::*,
};
use workspace::Workspace;
use workspace::notifications::DetachAndPromptErr;

/// Pure filter helper (unit-tested).
pub(crate) fn filter_branches(branches: &[Branch], query: &str) -> Vec<Branch> {
    if query.is_empty() {
        return branches.to_vec();
    }
    let query_lower = query.to_lowercase();
    branches
        .iter()
        .filter(|b| b.name().to_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}

#[cfg(test)]
pub(crate) fn sort_branches(branches: &mut [Branch]) {
    sort_branches_with_favorites(branches, &collections::HashSet::default());
}

pub(crate) fn sort_branches_with_favorites(
    branches: &mut [Branch],
    favorites: &collections::HashSet<String>,
) {
    branches.sort_by(|a, b| {
        let a_fav = favorites.contains(a.name());
        let b_fav = favorites.contains(b.name());
        b.is_head
            .cmp(&a.is_head)
            .then_with(|| b_fav.cmp(&a_fav))
            .then_with(|| a.is_remote().cmp(&b.is_remote()))
            .then_with(|| a.name().cmp(b.name()))
    });
}

pub(crate) fn load_branches_from_repo_with_favorites(
    repo: &Entity<Repository>,
    favorites: &collections::HashSet<String>,
    cx: &App,
) -> Vec<Branch> {
    let mut branches: Vec<Branch> = repo.read(cx).branch_list.iter().cloned().collect();
    sort_branches_with_favorites(&mut branches, favorites);
    branches
}

pub(crate) fn checkout_branch(
    repo: Entity<Repository>,
    branch: &Branch,
    window: &mut Window,
    cx: &mut App,
) {
    if branch.is_head {
        return;
    }
    let name = branch.name().to_string();
    let receiver = repo.update(cx, |repo, _| repo.change_branch(name));
    window
        .spawn(cx, async move |_cx| {
            receiver.await??;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Failed to change branch", cx),
            window,
            cx,
            |_, _, _| None,
        );
}

pub(crate) fn delete_branch_with_prompt(
    repo: Entity<Repository>,
    branch: Branch,
    window: &mut Window,
    cx: &mut App,
) {
    if branch.is_head {
        return;
    }
    let is_remote = branch.is_remote();
    let branch_name = branch.name().to_string();
    
    window
        .spawn(cx, async move |cx| {
            let answer = cx.update(|window, cx| {
                let prompt_msg = if is_remote {
                    format!(
                        "{} '{}'?",
                        translate_ui("Delete remote branch", cx),
                        branch_name
                    )
                } else {
                    format!("{} '{}'?", translate_ui("Delete branch", cx), branch_name)
                };
                let buttons = [translate_ui("Delete", cx), translate_ui("Cancel", cx)];
                window.prompt(PromptLevel::Warning, &prompt_msg, None, &buttons, cx)
            })?;

            if answer.await != Ok(0) {
                return anyhow::Ok(());
            }

            let result = repo
                .update(cx, |repo, _| {
                    repo.delete_branch(is_remote, branch_name.clone(), false)
                })
                .await?;

            match result {
                Ok(_) => anyhow::Ok(()),
                Err(error) => {
                    let should_force = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Warning,
                            &format!("{error}\n\nForce delete branch '{branch_name}'?"),
                            None,
                            &[translate_ui("Force Delete", cx), translate_ui("Cancel", cx)],
                            cx,
                        )
                    })?;
                    if should_force.await == Ok(0) {
                        repo.update(cx, |repo, _| {
                            repo.delete_branch(is_remote, branch_name, true)
                        })
                        .await??;
                        anyhow::Ok(())
                    } else {
                        Err(error)
                    }
                }
            }
        })
        .detach_and_prompt_err(
            translate_ui("Failed to delete branch", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

pub(crate) fn render_branch_list(
    branches: Vec<Branch>,
    has_repo: bool,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
    manager: gpui::WeakEntity<crate::git_manager::GitManager>,
    favorites: collections::HashSet<String>,
    cx: &App,
) -> AnyElement {
    if !has_repo {
        return Label::new(translate_ui("Open a folder with a git repository", cx))
            .color(Color::Muted)
            .into_any_element();
    }
    if branches.is_empty() {
        return Label::new(translate_ui("No branches match", cx))
            .color(Color::Muted)
            .into_any_element();
    }

    let branch_count = branches.len();
    uniform_list(
        "git-manager-branches",
        branch_count,
        move |range, _window, cx| {
            branches[range.clone()]
                .iter()
                .enumerate()
                .map(|(offset, branch)| {
                    let index = range.start + offset;
                    let is_fav = favorites.contains(branch.name());
                    render_branch_row(
                        index,
                        branch.clone(),
                        repo.clone(),
                        workspace.clone(),
                        manager.clone(),
                        is_fav,
                        cx,
                    )
                })
                .collect()
        },
    )
    .flex_1()
    .size_full()
    .into_any_element()
}

fn render_branch_row(
    index: usize,
    branch: Branch,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
    manager: gpui::WeakEntity<crate::git_manager::GitManager>,
    is_favorite: bool,
    cx: &App,
) -> AnyElement {
    let name = branch.name().to_string();
    let is_head = branch.is_head;
    let is_remote = branch.is_remote();
    let tracking = branch.tracking_status();
    let status = match tracking {
        Some(status) if status.ahead > 0 || status.behind > 0 => {
            format!("↑{} ↓{}", status.ahead, status.behind)
        }
        _ => String::new(),
    };

    let id = SharedString::from(format!("gm-branch-row-{index}"));
    let branch_for_click = branch.clone();
    let branch_for_menu = branch;
    let repo_for_click = repo.clone();
    let fav_manager = manager.clone();
    let fav_name = name.clone();

    h_flex()
        .id(ElementId::Name(id))
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().colors().element_hover))
        .when(is_head, |s| {
            s.bg(cx.theme().colors().element_selected.opacity(0.4))
        })
        .when(is_head, |this| {
            this.child(
                Label::new("✓")
                    .size(LabelSize::Small)
                    .color(Color::Accent)
                    .weight(FontWeight::BOLD),
            )
        })
        .child(
            Label::new(name)
                .size(LabelSize::Small)
                .when(is_head, |l| l.weight(FontWeight::SEMIBOLD)),
        )
        .when(is_remote, |this| {
            this.child(
                Label::new(translate_ui("remote", cx))
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
        })
        .when(!status.is_empty(), |this| {
            this.child(
                Label::new(status)
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
        })
        .child(div().flex_1())
        .child(
            IconButton::new(format!("gm-fav-btn-{index}"), IconName::Star)
                .icon_size(IconSize::XSmall)
                .when(is_favorite, |b| b.icon_color(Color::Warning))
                .when(!is_favorite, |b| b.icon_color(Color::Muted))
                .tooltip(Tooltip::text(if is_favorite {
                    translate_ui("Remove from Favorites", cx)
                } else {
                    translate_ui("Add to Favorites", cx)
                }))
                .on_click(move |_, _window, cx| {
                    if let Some(manager) = fav_manager.upgrade() {
                        manager.update(cx, |manager, cx| {
                            manager.toggle_favorite_branch(fav_name.clone(), cx);
                        });
                    }
                }),
        )
        .on_click({
            let branch = branch_for_click;
            let repo = repo_for_click;
            move |_, window, cx| {
                let Some(repo) = repo.clone() else {
                    return;
                };
                checkout_branch(repo, &branch, window, cx);
            }
        })
        .child(
            PopoverMenu::new(format!("gm-branch-menu-{index}"))
                .trigger(
                    IconButton::new(format!("gm-branch-menu-btn-{index}"), IconName::Ellipsis)
                        .icon_size(IconSize::XSmall),
                )
                .menu({
                    let branch = branch_for_menu;
                    let repo = repo;
                    let workspace = workspace;
                    let manager = manager;
                    move |window, cx| {
                        let branch = branch.clone();
                        let name_for_copy = branch.name().to_string();
                        let repo = repo.clone();
                        let workspace = workspace.clone();
                        let manager = manager.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let checkout_branch_item = branch.clone();
                            let checkout_repo = repo.clone();
                            let is_head = branch.is_head;
                            let is_remote = branch.is_remote();

                            let mut menu = menu;

                            menu = menu.entry(translate_ui("Checkout", cx), None, {
                                let branch = checkout_branch_item;
                                let repo = checkout_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    checkout_branch(repo, &branch, window, cx);
                                }
                            });

                            if is_remote {
                                let branch_name = branch.name().to_string();
                                let suggested_name = suggested_local_branch_name(&branch_name);
                                let repo = repo.clone();
                                let workspace = workspace.clone();
                                menu = menu.entry(
                                    translate_ui("Checkout as New Local Branch…", cx),
                                    None,
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        let branch_name = branch_name.clone();
                                        let suggested_name = suggested_name.clone();
                                        if let Some(workspace) = workspace.upgrade() {
                                            workspace.update(cx, |workspace, cx| {
                                                let title = format!(
                                                    "{} ({})",
                                                    translate_ui(
                                                        "Checkout as New Local Branch",
                                                        cx
                                                    ),
                                                    branch_name
                                                );
                                                workspace.toggle_modal(window, cx, |window, cx| {
                                                    crate::NewBranchModal::new(
                                                        title,
                                                        suggested_name,
                                                        Some(branch_name),
                                                        repo,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            });
                                        }
                                    },
                                );
                            }

                            {
                                let branch_name = branch.name().to_string();
                                let repo = repo.clone();
                                let workspace = workspace.clone();
                                menu = menu.entry(
                                    translate_ui("New Branch from Here…", cx),
                                    None,
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        let branch_name = branch_name.clone();
                                        if let Some(workspace) = workspace.upgrade() {
                                            workspace.update(cx, |workspace, cx| {
                                                let title = format!(
                                                    "{} ({})",
                                                    translate_ui("New Branch from Here", cx),
                                                    branch_name
                                                );
                                                workspace.toggle_modal(window, cx, |window, cx| {
                                                    crate::NewBranchModal::new(
                                                        title,
                                                        String::new(),
                                                        Some(branch_name),
                                                        repo,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            });
                                        }
                                    },
                                );
                            }

                            {
                                let favorite_name = branch.name().to_string();
                                let manager_fav = manager.clone();
                                let fav_label = if is_favorite {
                                    translate_ui("Remove from Favorites", cx)
                                } else {
                                    translate_ui("Add to Favorites", cx)
                                };
                                menu = menu.entry(fav_label, None, move |_window, cx| {
                                    if let Some(manager) = manager_fav.upgrade() {
                                        manager.update(cx, |manager, cx| {
                                            manager.toggle_favorite_branch(
                                                favorite_name.clone(),
                                                cx,
                                            );
                                        });
                                    }
                                });
                            }

                            menu = menu
                                .separator()
                                .entry(translate_ui("Compare with Current", cx), None, {
                                    let branch_name = branch.name().to_string();
                                    let repo = repo.clone();
                                    let workspace = workspace.clone();
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        let branch_name = branch_name.clone();
                                        if let Some(workspace) = workspace.upgrade() {
                                            workspace.update(cx, |workspace, cx| {
                                                let project = workspace.project().clone();
                                                crate::branch_diff::BranchDiff::deploy_branch_diff_with_base_ref(
                                                    workspace,
                                                    project,
                                                    repo,
                                                    branch_name.into(),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                })
                                .entry(translate_ui("Compare with…", cx), None, {
                                    let repo = repo.clone();
                                    let workspace = workspace.clone();
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        if let Some(workspace) = workspace.upgrade() {
                                            let workspace_handle = workspace.clone();
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.toggle_modal(window, cx, |window, cx| {
                                                    crate::RefPickerModal::new(
                                                        repo,
                                                        workspace_handle,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            });
                                        }
                                    }
                                })
                                .entry(translate_ui("Merge into Current", cx), None, {
                                    let branch = branch.clone();
                                    let repo = repo.clone();
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        crate::git_manager::operations::merge_branch(
                                            &repo,
                                            branch.name().to_string(),
                                            window,
                                            cx,
                                        );
                                    }
                                })
                                .entry(translate_ui("Rebase Current onto This", cx), None, {
                                    let branch = branch.clone();
                                    let repo = repo.clone();
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        crate::git_manager::operations::rebase_branch(
                                            &repo,
                                            branch.name().to_string(),
                                            window,
                                            cx,
                                        );
                                    }
                                });

                            if !is_remote {
                                let branch_name = branch.name().to_string();
                                let repo = repo.clone();
                                let workspace = workspace.clone();
                                menu = menu.entry(translate_ui("Rename Branch…", cx), None, {
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        let branch_name = branch_name.clone();
                                        if let Some(workspace) = workspace.upgrade() {
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.toggle_modal(window, cx, |window, cx| {
                                                    crate::RenameBranchModal::new(
                                                        branch_name,
                                                        repo,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            });
                                        }
                                    }
                                });
                            }

                            if !is_head {
                                let branch = branch.clone();
                                let repo = repo.clone();
                                menu = menu.entry(translate_ui("Delete Branch", cx), None, {
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        delete_branch_with_prompt(
                                            repo,
                                            branch.clone(),
                                            window,
                                            cx,
                                        );
                                    }
                                });
                            }

                            menu.separator().entry(
                                translate_ui("Copy Branch Name", cx),
                                None,
                                {
                                    let name = name_for_copy;
                                    move |_, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            name.clone(),
                                        ));
                                    }
                                },
                            )
                        }))
                    }
                }),
        )
        .into_any_element()
}

/// Build a single-line filter editor for the Branches section.
pub(crate) fn new_filter_editor(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    super::filter::new_filter_editor_with_placeholder(
        translate_ui("Filter branches…", cx),
        window,
        cx,
    )
}

pub(crate) fn subscribe_filter_edits(
    filter_editor: &Entity<Editor>,
    cx: &mut Context<crate::git_manager::GitManager>,
) -> gpui::Subscription {
    cx.subscribe(filter_editor, |this, _editor, event, cx| {
        if let EditorEvent::Edited { .. } = event {
            this.refresh_filtered_branches(cx);
        }
    })
}

/// Strip remote prefix to suggest a clean local branch name (e.g. "origin/feature" -> "feature").
pub(crate) fn suggested_local_branch_name(branch_name: &str) -> String {
    if let Some((_remote, local)) = branch_name.split_once('/') {
        local.to_string()
    } else {
        branch_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{filter_branches, sort_branches, suggested_local_branch_name};
    use git::repository::Branch;
    use gpui::SharedString;

    fn branch(name: &str, is_head: bool, remote: bool) -> Branch {
        Branch {
            is_head,
            ref_name: SharedString::from(if remote {
                format!("refs/remotes/origin/{name}")
            } else {
                format!("refs/heads/{name}")
            }),
            upstream: None,
            most_recent_commit: None,
        }
    }

    #[test]
    fn filter_branches_matches_substring() {
        let branches = vec![
            branch("main", true, false),
            branch("feature/git-manager", false, false),
            branch("develop", false, true),
        ];
        let filtered = filter_branches(&branches, "git");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), "feature/git-manager");
    }

    #[test]
    fn filter_branches_empty_query_returns_all() {
        let branches = vec![branch("main", true, false), branch("dev", false, false)];
        assert_eq!(filter_branches(&branches, "").len(), 2);
    }

    #[test]
    fn filter_branches_is_case_insensitive() {
        let branches = vec![branch("Main", true, false), branch("feature", false, false)];
        let filtered = filter_branches(&branches, "main");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), "Main");
    }

    #[test]
    fn sort_branches_puts_head_first_then_local() {
        let mut branches = vec![
            branch("z-remote", false, true),
            branch("a-local", false, false),
            branch("main", true, false),
        ];
        sort_branches(&mut branches);
        assert!(branches[0].is_head);
        assert_eq!(branches[0].name(), "main");
        assert!(!branches[1].is_remote());
        assert!(branches[2].is_remote());
    }

    #[test]
    fn sort_branches_puts_head_first_then_favorites_then_local() {
        let mut branches = vec![
            branch("z-remote", false, true),
            branch("a-local", false, false),
            branch("fav-branch", false, false),
            branch("main", true, false),
        ];
        let mut favorites = collections::HashSet::default();
        favorites.insert("fav-branch".to_string());
        super::sort_branches_with_favorites(&mut branches, &favorites);
        assert!(branches[0].is_head);
        assert_eq!(branches[0].name(), "main");
        assert_eq!(branches[1].name(), "fav-branch");
        assert_eq!(branches[2].name(), "a-local");
        assert!(branches[3].is_remote());
    }

    #[test]
    fn suggested_local_branch_name_strips_remote() {
        assert_eq!(suggested_local_branch_name("origin/main"), "main");
        assert_eq!(
            suggested_local_branch_name("upstream/feature/auth"),
            "feature/auth"
        );
        assert_eq!(suggested_local_branch_name("local-branch"), "local-branch");
    }
}

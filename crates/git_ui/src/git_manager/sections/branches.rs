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
use util::ResultExt;
use workspace::Workspace;
use workspace::notifications::DetachAndPromptErr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum BranchScope {
    #[default]
    All,
    Local,
    Remote,
}

/// Pure filter helper (unit-tested).
#[cfg(test)]
pub(crate) fn filter_branches(branches: &[Branch], query: &str) -> Vec<Branch> {
    filter_branches_with_favorites(
        branches,
        query,
        false,
        &collections::HashSet::default(),
        BranchScope::All,
    )
}

pub(crate) fn filter_branches_with_favorites(
    branches: &[Branch],
    query: &str,
    favorites_only: bool,
    favorites: &collections::HashSet<String>,
    scope: BranchScope,
) -> Vec<Branch> {
    let query_lower = query.to_lowercase();
    branches
        .iter()
        .filter(|b| match scope {
            BranchScope::All => true,
            BranchScope::Local => !b.is_remote(),
            BranchScope::Remote => b.is_remote(),
        })
        .filter(|b| {
            if favorites_only && !favorites.contains(b.name()) {
                return false;
            }
            if query.is_empty() {
                true
            } else {
                b.name().to_lowercase().contains(&query_lower)
            }
        })
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
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    if branch.is_head {
        return;
    }
    let name = branch.name().to_string();
    let receiver = repo.update(cx, |repo, _| repo.change_branch(name));
    window
        .spawn(cx, async move |cx| {
            receiver.await??;
            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_branches(cx);
                    this.refresh_in_progress_state(cx);
                });
            }
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
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
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
                Ok(_) => {
                    if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                        manager.update(cx, |this, cx| {
                            this.reload_branches(cx);
                            this.reload_remotes(cx);
                        });
                    }
                    anyhow::Ok(())
                }
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
                        if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                            manager.update(cx, |this, cx| {
                                this.reload_branches(cx);
                                this.reload_remotes(cx);
                            });
                        }
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

pub(crate) fn open_create_pull_request_url_for_branch(
    repo: &Entity<Repository>,
    branch: &git::repository::Branch,
    window: &mut Window,
    cx: &mut App,
) {
    let repo = repo.clone();
    let branch = branch.clone();
    window
        .spawn(cx, async move |cx| {
            let (source_branch, target_remote) = if branch.is_remote() {
                let remote_name = branch.remote_name().unwrap_or("origin");
                let branch_name = suggested_local_branch_name(branch.name());
                (branch_name, Some(remote_name.to_string()))
            } else {
                let branch_name = branch.name().to_string();
                let remote_name = branch
                    .upstream
                    .as_ref()
                    .and_then(|u| u.remote_name())
                    .map(|s| s.to_string());
                (branch_name, remote_name)
            };

            let remote_url = cx.update(|_window, cx| {
                repo.read_with(cx, |snapshot, _| {
                    target_remote
                        .as_ref()
                        .and_then(|name| snapshot.remotes.get(name).cloned())
                        .or_else(|| {
                            if snapshot.remotes.contains_key("origin") {
                                snapshot.remotes.get("origin").cloned()
                            } else if snapshot.remotes.contains_key("upstream") {
                                snapshot.remotes.get("upstream").cloned()
                            } else {
                                snapshot.remotes.values().next().cloned()
                            }
                        })
                        .or_else(|| snapshot.remote_origin_url.clone())
                        .or_else(|| snapshot.remote_upstream_url.clone())
                })
            })?
            .ok_or_else(|| anyhow::anyhow!("No remote configured for repository"))?;

            let (url, _) = cx.update(|_window, cx| {
                let provider_registry = git::GitHostingProviderRegistry::default_global(cx);
                let (provider, parsed_remote) = git::parse_git_remote_url(provider_registry, &remote_url)
                    .ok_or_else(|| anyhow::anyhow!("Unsupported remote URL: {remote_url}"))?;
                let url = provider
                    .build_create_pull_request_url(&parsed_remote, &source_branch)
                    .ok_or_else(|| anyhow::anyhow!("Unable to construct pull request URL"))?;
                cx.open_url(url.as_str());
                anyhow::Ok((url, ()))
            })??;

            let _ = url;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Failed to create pull request", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
}

pub(crate) fn delete_merged_branches_with_prompt(
    repo: Entity<Repository>,
    manager: Option<gpui::WeakEntity<crate::git_manager::GitManager>>,
    window: &mut Window,
    cx: &mut App,
) {
    window
        .spawn(cx, async move |cx| {
            let receiver = repo.update(cx, |repo, _| repo.merged_branches());
            let merged = receiver.await??;
            if merged.is_empty() {
                cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Info,
                        translate_ui("No merged branches found", cx),
                        Some(translate_ui(
                            "All local branches have unmerged changes or are currently active.",
                            cx,
                        )),
                        &[translate_ui("OK", cx)],
                        cx,
                    )
                })?
                .await
                .ok();
                return anyhow::Ok(());
            }

            let answer = cx.update(|window, cx| {
                let branch_count = merged.len();
                let branch_list_str = merged
                    .iter()
                    .map(|b| format!("• {b}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let prompt_title = if branch_count == 1 {
                    translate_ui("Delete 1 merged branch?", cx).to_string()
                } else {
                    format!(
                        "{} {} {}?",
                        translate_ui("Delete", cx),
                        branch_count,
                        translate_ui("merged branches", cx)
                    )
                };
                let prompt_detail = format!(
                    "{}\n\n{}",
                    translate_ui(
                        "The following branches are fully merged into HEAD and will be safely deleted:",
                        cx
                    ),
                    branch_list_str
                );
                let delete_label = if branch_count == 1 {
                    translate_ui("Delete Branch", cx).to_string()
                } else {
                    format!(
                        "{} {} {}",
                        translate_ui("Delete", cx),
                        branch_count,
                        translate_ui("Branches", cx)
                    )
                };
                let buttons = [delete_label.as_str(), translate_ui("Cancel", cx)];
                window.prompt(
                    PromptLevel::Warning,
                    &prompt_title,
                    Some(&prompt_detail),
                    &buttons,
                    cx,
                )
            })?;

            if answer.await != Ok(0) {
                return anyhow::Ok(());
            }

            for branch_name in merged {
                let rx = repo.update(cx, |repo, _| {
                    repo.delete_branch(false, branch_name, false)
                });
                rx.await?.log_err();
            }

            if let Some(manager) = manager.and_then(|m| m.upgrade()) {
                manager.update(cx, |this, cx| {
                    this.reload_branches(cx);
                });
            }

            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Failed to clean merged branches", cx),
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
        return v_flex()
            .w_full()
            .items_center()
            .py_6()
            .child(
                Label::new(translate_ui("Open a folder with a git repository", cx))
                    .color(Color::Muted),
            )
            .into_any_element();
    }
    if branches.is_empty() {
        return v_flex()
            .w_full()
            .items_center()
            .py_6()
            .child(Label::new(translate_ui("No branches match", cx)).color(Color::Muted))
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
    let (ahead, behind) = match branch.tracking_status() {
        Some(status) => (status.ahead, status.behind),
        None => (0, 0),
    };

    let (branch_commit_tip, commit_relative_time) = match branch.most_recent_commit.as_ref() {
        Some(c) => {
            let now = time::OffsetDateTime::now_utc();
            let local_offset =
                time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
            let date_str = crate::format_git_commit_timestamp(
                c.commit_timestamp,
                now,
                local_offset,
                time_format::TimestampFormat::Absolute,
                cx,
            )
            .unwrap_or_default();
            let rel_time = crate::format_git_commit_timestamp(
                c.commit_timestamp,
                now,
                local_offset,
                time_format::TimestampFormat::Relative,
                cx,
            );
            let short_sha: String = c.sha.chars().take(8).collect();
            (
                Some(format!(
                    "{}\n{} • {}\nSHA: {}",
                    c.subject, c.author_name, date_str, short_sha
                )),
                rel_time,
            )
        }
        None => (None, None),
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
            h_flex()
                .id(ElementId::Name(format!("gm-branch-name-{index}").into()))
                .child(
                    Label::new(name)
                        .size(LabelSize::Small)
                        .when(is_head, |l| l.weight(FontWeight::SEMIBOLD)),
                )
                .when_some(branch_commit_tip, |this, tip| {
                    this.tooltip(Tooltip::text(tip))
                }),
        )
        .when(is_remote, |this| {
            this.child(
                Label::new(translate_ui("remote", cx))
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
        })
        .when(ahead > 0, |this| {
            let tip = if ahead == 1 {
                format!("1 {}", translate_ui("commit ahead of upstream", cx))
            } else {
                format!("{ahead} {}", translate_ui("commits ahead of upstream", cx))
            };
            this.child(
                h_flex()
                    .id(ElementId::Name(format!("gm-ahead-{index}").into()))
                    .child(
                        Label::new(format!("↑{ahead}"))
                            .size(LabelSize::XSmall)
                            .color(Color::Accent),
                    )
                    .tooltip(Tooltip::text(tip)),
            )
        })
        .when(behind > 0, |this| {
            let tip = if behind == 1 {
                format!("1 {}", translate_ui("commit behind upstream", cx))
            } else {
                format!("{behind} {}", translate_ui("commits behind upstream", cx))
            };
            this.child(
                h_flex()
                    .id(ElementId::Name(format!("gm-behind-{index}").into()))
                    .child(
                        Label::new(format!("↓{behind}"))
                            .size(LabelSize::XSmall)
                            .color(Color::Warning),
                    )
                    .tooltip(Tooltip::text(tip)),
            )
        })
        .child(div().flex_1())
        .when_some(commit_relative_time, |this, time| {
            this.child(
                Label::new(time)
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
        })
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
            let manager = manager.clone();
            move |_, window, cx| {
                let Some(repo) = repo.clone() else {
                    return;
                };
                checkout_branch(repo, &branch, Some(manager.clone()), window, cx);
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

                            if !is_head {
                                menu = menu.entry(translate_ui("Checkout", cx), None, {
                                    let branch = checkout_branch_item;
                                    let repo = checkout_repo;
                                    let manager = manager.clone();
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        checkout_branch(repo, &branch, Some(manager.clone()), window, cx);
                                    }
                                });
                            }

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

                            if !is_remote {
                                let push_branch_name = branch.name().to_string();
                                let push_repo = repo.clone();
                                let push_workspace = workspace.clone();
                                let push_manager = manager.clone();
                                menu = menu.entry(
                                    translate_ui("Push Branch to Remote", cx),
                                    None,
                                    move |window, cx| {
                                        let Some(repo) = push_repo.clone() else {
                                            return;
                                        };
                                        let remote_name = repo.read_with(cx, |repo, _| {
                                            if repo.remotes.contains_key("origin") {
                                                Some("origin".to_string())
                                            } else {
                                                repo.remotes.keys().next().cloned()
                                            }
                                        });
                                        let Some(remote_name) = remote_name else {
                                            window
                                                .spawn(cx, async move |_| -> anyhow::Result<()> {
                                                    anyhow::bail!("No remote configured to push branch to");
                                                })
                                                .detach_and_prompt_err(
                                                    translate_ui("Failed to push branch", cx),
                                                    window,
                                                    cx,
                                                    |e, _, _| Some(e.to_string()),
                                                );
                                            return;
                                        };
                                        let branch_name = push_branch_name.clone();
                                        let askpass = crate::git_manager::operations::askpass_delegate(
                                            &push_workspace,
                                            format!("git push {remote_name} {branch_name}"),
                                            window,
                                            cx,
                                        );
                                        let receiver = repo.update(cx, |repo, cx| {
                                            repo.push(
                                                branch_name.clone().into(),
                                                branch_name.clone().into(),
                                                remote_name.into(),
                                                Some(git::repository::PushOptions::SetUpstream),
                                                askpass,
                                                cx,
                                            )
                                        });
                                        let manager_handle = push_manager.clone();
                                        window
                                            .spawn(cx, async move |cx| {
                                                receiver.await??;
                                                if let Some(manager) = manager_handle.upgrade() {
                                                    manager.update(cx, |this, cx| {
                                                        this.reload_branches(cx);
                                                        this.refresh_in_progress_state(cx);
                                                    });
                                                }
                                                anyhow::Ok(())
                                            })
                                            .detach_and_prompt_err(
                                                translate_ui("Failed to push branch", cx),
                                                window,
                                                cx,
                                                |e, _, _| Some(e.to_string()),
                                            );
                                    },
                                );
                            }

                            {
                                let pr_branch = branch.clone();
                                let pr_repo = repo.clone();
                                menu = menu.entry(
                                    translate_ui("Create Pull Request…", cx),
                                    None,
                                    move |window, cx| {
                                        let Some(repo) = pr_repo.clone() else {
                                            return;
                                        };
                                        open_create_pull_request_url_for_branch(
                                            &repo,
                                            &pr_branch,
                                            window,
                                            cx,
                                        );
                                    },
                                );
                            }

                            if !is_head {
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
                                    .entry(translate_ui("Compare in Git Graph", cx), None, {
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
                                                    let git_store =
                                                        workspace.project().read(cx).git_store().clone();
                                                    let repo_id = repo.read(cx).id;
                                                    crate::git_graph::open_or_reuse_graph_for_branch_compare(
                                                        workspace,
                                                        repo_id,
                                                        git_store,
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
                                        let workspace = workspace.clone();
                                        let manager = manager.clone();
                                        move |window, cx| {
                                            let Some(repo) = repo.clone() else {
                                                return;
                                            };
                                            crate::git_manager::operations::merge_branch(
                                                &repo,
                                                branch.name().to_string(),
                                                Some(workspace.clone()),
                                                Some(manager.clone()),
                                                window,
                                                cx,
                                            );
                                        }
                                    })
                                    .entry(translate_ui("Rebase Current onto This", cx), None, {
                                        let branch = branch.clone();
                                        let repo = repo.clone();
                                        let workspace = workspace.clone();
                                        let manager = manager.clone();
                                        move |window, cx| {
                                            let Some(repo) = repo.clone() else {
                                                return;
                                            };
                                            crate::git_manager::operations::rebase_branch(
                                                &repo,
                                                branch.name().to_string(),
                                                Some(workspace.clone()),
                                                Some(manager.clone()),
                                                window,
                                                cx,
                                            );
                                        }
                                    });
                            } else {
                                menu = menu
                                    .separator()
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
                                    });
                            }

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

                            {
                                let branch_name = branch.name().to_string();
                                let repo = repo.clone();
                                let workspace = workspace.clone();
                                let manager = manager.clone();
                                menu = menu.entry(translate_ui("New Tag on Branch…", cx), None, move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let branch_name = branch_name.clone();
                                    let manager = manager.clone();
                                    if let Some(workspace) = workspace.upgrade() {
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.toggle_modal(window, cx, |window, cx| {
                                                crate::git_manager::sections::NewTagModal::new(
                                                    repo,
                                                    Some(branch_name),
                                                    Some(manager),
                                                    window,
                                                    cx,
                                                )
                                            });
                                        });
                                    }
                                });
                            }

                            if is_remote {
                                let branch_name = branch.name().to_string();
                                let repo = repo.clone();
                                let workspace = workspace.clone();
                                let manager = manager.clone();
                                menu = menu.entry(translate_ui("Fetch Remote Branch", cx), None, move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let branch_name = branch_name.clone();
                                    let manager = manager.clone();
                                    let remote_name = branch_name.split('/').next().unwrap_or("origin").to_string();
                                    let askpass = crate::git_manager::operations::askpass_delegate(
                                        &workspace,
                                        format!("git fetch {remote_name}"),
                                        window,
                                        cx,
                                    );
                                    let receiver = repo.update(cx, |repo, cx| {
                                        repo.fetch(
                                            git::repository::FetchOptions::Remote(git::repository::Remote {
                                                name: remote_name.into(),
                                            }),
                                            askpass,
                                            cx,
                                        )
                                    });
                                    window
                                        .spawn(cx, async move |cx| {
                                            receiver.await??;
                                            if let Some(manager) = manager.upgrade() {
                                                manager.update(cx, |this, cx| {
                                                    this.reload_branches(cx);
                                                    this.reload_remotes(cx);
                                                });
                                            }
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to fetch branch", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                });
                            }

                            if !is_head {
                                let branch = branch.clone();
                                let repo = repo.clone();
                                let manager = manager.clone();
                                let delete_label = if is_remote {
                                    translate_ui("Delete Remote Branch", cx)
                                } else {
                                    translate_ui("Delete Branch", cx)
                                };
                                menu = menu.entry(delete_label, None, {
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        delete_branch_with_prompt(
                                            repo,
                                            branch.clone(),
                                            Some(manager.clone()),
                                            window,
                                            cx,
                                        );
                                    }
                                });
                            }

                            menu = menu.separator().entry(
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
                            );

                            if let Some(commit) = branch.most_recent_commit.as_ref() {
                                let sha = commit.sha.clone();
                                menu = menu.entry(
                                    translate_ui("Copy Commit SHA", cx),
                                    None,
                                    move |_, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            sha.to_string(),
                                        ));
                                    },
                                );
                            }

                            if let Some(commit) = branch.most_recent_commit.as_ref() {
                                let sha = commit.sha.to_string();
                                menu = menu.entry(
                                    translate_ui("Show in Git Graph", cx),
                                    None,
                                    move |window, cx| {
                                        window.dispatch_action(
                                            Box::new(crate::git_graph::OpenAtCommit {
                                                sha: sha.clone(),
                                            }),
                                            cx,
                                        );
                                    },
                                );
                            }

                            menu
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

    #[test]
    fn filter_branches_with_favorites_filters_correctly() {
        let branches = vec![
            branch("main", true, false),
            branch("feature/fav", false, false),
            branch("feature/other", false, false),
        ];
        let mut favorites = collections::HashSet::default();
        favorites.insert("feature/fav".to_string());

        let favs_only = super::filter_branches_with_favorites(
            &branches,
            "",
            true,
            &favorites,
            super::BranchScope::All,
        );
        assert_eq!(favs_only.len(), 1);
        assert_eq!(favs_only[0].name(), "feature/fav");

        let favs_with_query = super::filter_branches_with_favorites(
            &branches,
            "other",
            true,
            &favorites,
            super::BranchScope::All,
        );
        assert_eq!(favs_with_query.len(), 0);
    }

    #[test]
    fn filter_branches_with_scope_filters_correctly() {
        let branches = vec![
            branch("main", true, false),
            branch("origin/main", false, true),
            branch("origin/feature", false, true),
        ];
        let favorites = collections::HashSet::default();

        let all = super::filter_branches_with_favorites(
            &branches,
            "",
            false,
            &favorites,
            super::BranchScope::All,
        );
        assert_eq!(all.len(), 3);

        let local = super::filter_branches_with_favorites(
            &branches,
            "",
            false,
            &favorites,
            super::BranchScope::Local,
        );
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].name(), "main");

        let remote = super::filter_branches_with_favorites(
            &branches,
            "",
            false,
            &favorites,
            super::BranchScope::Remote,
        );
        assert_eq!(remote.len(), 2);
    }
}

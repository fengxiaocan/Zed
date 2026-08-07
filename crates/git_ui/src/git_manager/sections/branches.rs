use editor::{Editor, EditorEvent};
use git::repository::Branch;
use gpui::{
    Action, App, ClipboardItem, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, SharedString, Styled, Window, div, uniform_list,
};
use project::git_store::Repository;
use settings::translate_ui;
use ui::{
    Color, ContextMenu, IconButton, IconName, IconSize, Label, LabelSize, PopoverMenu, prelude::*,
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

pub(crate) fn sort_branches(branches: &mut [Branch]) {
    branches.sort_by(|a, b| {
        b.is_head
            .cmp(&a.is_head)
            .then_with(|| a.is_remote().cmp(&b.is_remote()))
            .then_with(|| a.name().cmp(b.name()))
    });
}

pub(crate) fn load_branches_from_repo(repo: &Entity<Repository>, cx: &App) -> Vec<Branch> {
    let mut branches: Vec<Branch> = repo.read(cx).branch_list.iter().cloned().collect();
    sort_branches(&mut branches);
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

pub(crate) fn render_branch_list(
    branches: Vec<Branch>,
    has_repo: bool,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
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
                    render_branch_row(index, branch.clone(), repo.clone(), workspace.clone(), cx)
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
        .child(
            Label::new(if is_head {
                format!("★ {name}")
            } else {
                name.clone()
            })
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
                    move |window, cx| {
                        let branch = branch.clone();
                        let name_for_copy = branch.name().to_string();
                        let repo = repo.clone();
                        let workspace = workspace.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let checkout_branch_item = branch.clone();
                            let checkout_repo = repo.clone();
                            let create_workspace = workspace.clone();

                            menu.entry(translate_ui("Checkout", cx), None, {
                                let branch = checkout_branch_item;
                                let repo = checkout_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    checkout_branch(repo, &branch, window, cx);
                                }
                            })
                            .entry(translate_ui("New Branch from Here", cx), None, {
                                let workspace = create_workspace;
                                move |window, cx| {
                                    // Reuse the existing Branch picker for naming; keeps create UX consistent.
                                    if let Some(workspace) = workspace.upgrade() {
                                        workspace.update(cx, |workspace, cx| {
                                            window.focus(&workspace.focus_handle(cx), cx);
                                            window.dispatch_action(
                                                zed_actions::git::Branch.boxed_clone(),
                                                cx,
                                            );
                                        });
                                    }
                                }
                            })
                            .separator()
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
                            })
                            .separator()
                            .entry(
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

#[cfg(test)]
mod tests {
    use super::{filter_branches, sort_branches};
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
}

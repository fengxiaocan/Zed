use git::repository::RepoPath;
use gpui::{
    actions, App, ClickEvent, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, KeyContext, Render, ScrollHandle, Subscription, WeakEntity, Window, rems,
};
use project::git_store::{Repository, RepositoryEvent};
use settings::translate_ui;
use ui::prelude::*;
use util::paths::PathStyle;
use workspace::{ModalView, Workspace};

actions!(git_conflict, [Cancel, Confirm]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictedFileItem {
    pub repo_path: RepoPath,
    pub display_name: String,
    pub relative_path: String,
    pub is_resolved: bool,
}

pub struct GitConflictModal {
    repo: Entity<Repository>,
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    is_rebase: bool,
    conflicted_files: Vec<ConflictedFileItem>,
    scroll_handle: ScrollHandle,
    _subscription: Subscription,
}

impl GitConflictModal {
    pub fn open(
        workspace: &mut Workspace,
        repo: Entity<Repository>,
        is_rebase: bool,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let workspace_handle = workspace.weak_handle();
        workspace.toggle_modal(window, cx, move |window, cx| {
            Self::new(repo, workspace_handle, is_rebase, window, cx)
        });
    }

    pub fn new(
        repo: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        is_rebase: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&repo, |this, _, _event: &RepositoryEvent, cx| {
            this.refresh_conflicts(cx);
            cx.notify();
        });

        let mut modal = Self {
            repo,
            workspace,
            focus_handle: cx.focus_handle(),
            is_rebase,
            conflicted_files: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            _subscription: subscription,
        };

        modal.refresh_conflicts(cx);
        modal
    }

    fn refresh_conflicts(&mut self, cx: &App) {
        let repo = self.repo.read(cx);
        let mut files = Vec::new();

        for status_entry in repo.status() {
            let is_conflicted = status_entry.status.is_conflicted()
                || repo.had_conflict_on_last_merge_head_change(&status_entry.repo_path);
            if is_conflicted {
                let relative_path = status_entry.repo_path.display(PathStyle::local()).to_string();
                let display_name = status_entry
                    .repo_path
                    .file_name()
                    .map(|n| n.to_owned())
                    .unwrap_or_else(|| relative_path.clone());
                let is_resolved = !status_entry.status.is_conflicted();

                files.push(ConflictedFileItem {
                    repo_path: status_entry.repo_path,
                    display_name,
                    relative_path,
                    is_resolved,
                });
            }
        }

        self.conflicted_files = files;
    }

    fn cancel(&mut self, _: &menu::Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn open_file_for_editing(
        &mut self,
        repo_path: RepoPath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project_path) = self.repo.read(cx).repo_path_to_project_path(&repo_path, cx) else {
            return;
        };

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                workspace
                    .open_path_preview(project_path, None, false, false, true, window, cx)
                    .detach_and_log_err(cx);
            });
        }

        cx.emit(DismissEvent);
    }

    fn open_all_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let repo_read = self.repo.read(cx);
        let project_paths: Vec<_> = self
            .conflicted_files
            .iter()
            .filter_map(|file| repo_read.repo_path_to_project_path(&file.repo_path, cx))
            .collect();

        if let Some(workspace) = self.workspace.upgrade() {
            workspace.update(cx, |workspace, cx| {
                for project_path in project_paths {
                    workspace
                        .open_path_preview(project_path, None, false, false, true, window, cx)
                        .detach_and_log_err(cx);
                }
            });
        }

        cx.emit(DismissEvent);
    }

    fn stage_file(&mut self, repo_path: RepoPath, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let task = repo.update(cx, |repo, cx| repo.stage_entries(vec![repo_path], cx));
            task.await?;
            this.update(cx, |this, cx| {
                this.refresh_conflicts(cx);
                cx.notify();
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn unstage_file(&mut self, repo_path: RepoPath, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        cx.spawn(async move |this, cx| {
            let task = repo.update(cx, |repo, cx| repo.unstage_entries(vec![repo_path], cx));
            task.await?;
            this.update(cx, |this, cx| {
                this.refresh_conflicts(cx);
                cx.notify();
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn stage_all_conflicts(&mut self, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        let paths: Vec<RepoPath> = self
            .conflicted_files
            .iter()
            .filter(|f| !f.is_resolved)
            .map(|f| f.repo_path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let task = repo.update(cx, |repo, cx| repo.stage_entries(paths, cx));
            task.await?;
            this.update(cx, |this, cx| {
                this.refresh_conflicts(cx);
                cx.notify();
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn confirm_abort(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let repo = self.repo.clone();
        let is_rebase = self.is_rebase;
        let title = if is_rebase {
            translate_ui("Abort Rebase?", cx)
        } else {
            translate_ui("Abort Merge?", cx)
        };
        let message = if is_rebase {
            translate_ui(
                "All uncommitted changes from this rebase will be lost and the repository will be restored to its state before the pull.",
                cx,
            )
        } else {
            translate_ui(
                "All uncommitted changes from this merge will be lost and the repository will be restored to its state before the pull.",
                cx,
            )
        };
        let abort_label = if is_rebase {
            translate_ui("Abort Rebase", cx)
        } else {
            translate_ui("Abort Merge", cx)
        };
        let cancel_label = translate_ui("Cancel", cx);

        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            title,
            Some(message),
            &[abort_label, cancel_label],
            cx,
        );

        cx.spawn_in(window, async move |this, cx| {
            if answer.await == Ok(0) {
                if is_rebase {
                    let task = repo.update(cx, |repo, _| repo.rebase_abort());
                    task.await??;
                } else {
                    let task = repo.update(cx, |repo, _| repo.merge_abort());
                    task.await??;
                }
                this.update(cx, |_, cx| {
                    cx.emit(DismissEvent);
                })
                .ok();
            }
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn complete_merge(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_rebase {
            let repo = self.repo.clone();
            cx.spawn_in(window, async move |this, cx| {
                let task = repo.update(cx, |repo, _| repo.rebase_continue());
                task.await??;
                this.update(cx, |_, cx| {
                    cx.emit(DismissEvent);
                })
                .ok();
                anyhow::Ok(())
            })
            .detach_and_log_err(cx);
        } else {
            if let Some(workspace) = self.workspace.upgrade() {
                workspace.update(cx, |workspace, cx| {
                    if let Some(git_panel) = workspace.panel::<crate::git_panel::GitPanel>(cx) {
                        git_panel.update(cx, |panel, cx| {
                            let options = panel.commit_options();
                            panel.commit_changes(options, window, cx);
                        });
                    }
                });
            }
            cx.emit(DismissEvent);
        }
    }
}

impl EventEmitter<DismissEvent> for GitConflictModal {}
impl ModalView for GitConflictModal {}

impl Focusable for GitConflictModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GitConflictModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.is_rebase {
            translate_ui("Git Pull Conflicts (Rebase)", cx)
        } else {
            translate_ui("Git Pull Conflicts", cx)
        };

        let resolved_count = self.conflicted_files.iter().filter(|f| f.is_resolved).count();
        let total_count = self.conflicted_files.len();
        let all_resolved = total_count > 0 && resolved_count == total_count;

        v_flex()
            .key_context({
                let mut key_context = KeyContext::new_with_defaults();
                key_context.add("GitConflictModal");
                key_context
            })
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .w(rems(42.))
            .max_h(rems(34.))
            .elevation_3(cx)
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Warning)
                                    .color(Color::Warning)
                                    .size(IconSize::Medium),
                            )
                            .child(Headline::new(title).size(HeadlineSize::Small)),
                    )
                    .child(
                        IconButton::new("close-conflict-modal", IconName::Close)
                            .icon_size(IconSize::Small)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    ),
            )
            .child(
                v_flex()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().status().warning_border)
                    .bg(cx.theme().status().warning_background.opacity(0.12))
                    .gap_1p5()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(
                                Icon::new(IconName::Info)
                                    .size(IconSize::XSmall)
                                    .color(Color::Warning),
                            )
                            .child(
                                Label::new(translate_ui("How to resolve conflicts", cx))
                                    .size(LabelSize::Small)
                                    .weight(FontWeight::SEMIBOLD),
                            ),
                    )
                    .child(
                        Label::new(translate_ui(
                            "Pull operation was paused because of merge conflicts. Resolve them in the editor or abort the merge.",
                            cx,
                        ))
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Label::new(translate_ui(
                                    "Step 1: Click 'Edit' on each file below to open and resolve conflict markers in the editor.",
                                    cx,
                                ))
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                            )
                            .child(
                                Label::new(translate_ui(
                                    "Step 2: Save the file and stage it to mark as resolved.",
                                    cx,
                                ))
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                            )
                            .child(
                                Label::new(if self.is_rebase {
                                    translate_ui(
                                        "Step 3: Once all conflicts are resolved, continue the rebase in the Git panel.",
                                        cx,
                                    )
                                } else {
                                    translate_ui(
                                        "Step 3: Once all conflicts are resolved, complete the merge or commit in the Git panel.",
                                        cx,
                                    )
                                })
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                            ),
                    ),
            )
            .when(all_resolved, |this| {
                this.child(
                    h_flex()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().status().success_border)
                        .bg(cx.theme().status().success_background.opacity(0.12))
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(IconName::Check)
                                .size(IconSize::Small)
                                .color(Color::Success),
                        )
                        .child(
                            Label::new(if self.is_rebase {
                                translate_ui(
                                    "All conflicts resolved. You can now continue the rebase.",
                                    cx,
                                )
                            } else {
                                translate_ui(
                                    "All conflicts resolved. You can now complete the merge.",
                                    cx,
                                )
                            })
                            .size(LabelSize::Small)
                            .color(Color::Success),
                        ),
                )
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(translate_ui("Conflicted Files", cx))
                                            .size(LabelSize::Small)
                                            .weight(FontWeight::SEMIBOLD),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "({}/{} {})",
                                            resolved_count,
                                            total_count,
                                            translate_ui("Resolved", cx)
                                        ))
                                        .size(LabelSize::XSmall)
                                        .color(if all_resolved {
                                            Color::Success
                                        } else {
                                            Color::Muted
                                        }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        Button::new("open-all-conflicts", translate_ui("Open All", cx))
                                            .size(ButtonSize::Compact)
                                            .style(ButtonStyle::Subtle)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_all_files(window, cx);
                                            })),
                                    )
                                    .when(!all_resolved, |this| {
                                        this.child(
                                            Button::new("stage-all-conflicts", translate_ui("Stage All", cx))
                                                .size(ButtonSize::Compact)
                                                .style(ButtonStyle::Subtle)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.stage_all_conflicts(cx);
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("conflict-files-list")
                            .gap_1p5()
                            .max_h(rems(13.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .children(self.conflicted_files.iter().enumerate().map(|(ix, file)| {
                                let repo_path = file.repo_path.clone();
                                h_flex()
                                    .id(("conflict-file", ix))
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(cx.theme().colors().element_background)
                                    .hover(|style| style.bg(cx.theme().colors().element_hover))
                                    .cursor_pointer()
                                    .justify_between()
                                    .items_center()
                                    .gap_2()
                                    .on_click(cx.listener({
                                        let repo_path = repo_path.clone();
                                        move |this, _, window, cx| {
                                            this.open_file_for_editing(
                                                repo_path.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    }))
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .min_w_0()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::FileCode)
                                                    .size(IconSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .child(
                                                v_flex()
                                                    .min_w_0()
                                                    .child(
                                                        Label::new(file.display_name.clone())
                                                            .size(LabelSize::Small),
                                                    )
                                                    .when(file.display_name != file.relative_path, |this| {
                                                        this.child(
                                                            Label::new(file.relative_path.clone())
                                                                .size(LabelSize::XSmall)
                                                                .color(Color::Muted)
                                                                .truncate(),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .flex_shrink_0()
                                            .when(file.is_resolved, |this| {
                                                this.child(
                                                    h_flex()
                                                        .gap_1()
                                                        .items_center()
                                                        .child(
                                                            Icon::new(IconName::Check)
                                                                .size(IconSize::XSmall)
                                                                .color(Color::Success),
                                                        )
                                                        .child(
                                                            Label::new(translate_ui("Resolved", cx))
                                                                .size(LabelSize::XSmall)
                                                                .color(Color::Success),
                                                        ),
                                                )
                                            })
                                            .when(!file.is_resolved, |this| {
                                                this.child(
                                                    h_flex()
                                                        .gap_1()
                                                        .items_center()
                                                        .child(
                                                            Icon::new(IconName::GitMergeConflict)
                                                                .size(IconSize::XSmall)
                                                                .color(Color::Warning),
                                                        )
                                                        .child(
                                                            Label::new(translate_ui(
                                                                "Unresolved",
                                                                cx,
                                                            ))
                                                            .size(LabelSize::XSmall)
                                                            .color(Color::Warning),
                                                        ),
                                                )
                                            })
                                            .child(
                                                Button::new(("edit-conflict", ix), translate_ui("Edit", cx))
                                                    .size(ButtonSize::Compact)
                                                    .style(ButtonStyle::OutlinedGhost)
                                                    .on_click(cx.listener({
                                                        let repo_path = repo_path.clone();
                                                        move |this, _, window, cx| {
                                                            this.open_file_for_editing(
                                                                repo_path.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                    })),
                                            )
                                            .when(!file.is_resolved, |this| {
                                                let repo_path = repo_path.clone();
                                                this.child(
                                                    Button::new(("stage-conflict", ix), translate_ui("Mark Resolved", cx))
                                                        .size(ButtonSize::Compact)
                                                        .style(ButtonStyle::OutlinedGhost)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.stage_file(repo_path.clone(), cx);
                                                        })),
                                                )
                                            })
                                            .when(file.is_resolved, |this| {
                                                let repo_path = repo_path.clone();
                                                this.child(
                                                    Button::new(("unstage-conflict", ix), translate_ui("Unstage", cx))
                                                        .size(ButtonSize::Compact)
                                                        .style(ButtonStyle::Subtle)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.unstage_file(repo_path.clone(), cx);
                                                        })),
                                                )
                                            }),
                                    )
                            })),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .pt_2()
                    .border_t_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(
                        Button::new(
                            "abort-conflict",
                            if self.is_rebase {
                                translate_ui("Abort Rebase", cx)
                            } else {
                                translate_ui("Abort Merge", cx)
                            },
                        )
                        .color(Color::Error)
                        .style(ButtonStyle::OutlinedGhost)
                        .size(ButtonSize::Compact)
                        .on_click(cx.listener(Self::confirm_abort)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("dismiss-conflict-modal", translate_ui("Resolve Later", cx))
                                    .style(ButtonStyle::OutlinedGhost)
                                    .size(ButtonSize::Compact)
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                            )
                            .child(
                                Button::new(
                                    "complete-merge",
                                    if self.is_rebase {
                                        translate_ui("Continue Rebase", cx)
                                    } else {
                                        translate_ui("Complete Merge", cx)
                                    },
                                )
                                    .style(if all_resolved {
                                        ButtonStyle::Filled
                                    } else {
                                        ButtonStyle::OutlinedGhost
                                    })
                                    .size(ButtonSize::Compact)
                                    .disabled(!all_resolved)
                                    .on_click(cx.listener(Self::complete_merge)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflicted_file_item_resolved_status() {
        let item1 = ConflictedFileItem {
            repo_path: RepoPath::new("src/main.rs").expect("valid repo path"),
            display_name: "main.rs".to_string(),
            relative_path: "src/main.rs".to_string(),
            is_resolved: false,
        };
        let item2 = ConflictedFileItem {
            repo_path: RepoPath::new("src/lib.rs").expect("valid repo path"),
            display_name: "lib.rs".to_string(),
            relative_path: "src/lib.rs".to_string(),
            is_resolved: true,
        };

        let items = [item1, item2];
        let resolved_count = items.iter().filter(|f| f.is_resolved).count();
        let total_count = items.len();

        assert_eq!(resolved_count, 1);
        assert_eq!(total_count, 2);
        assert!(!items.iter().all(|f| f.is_resolved));

        // When both are resolved:
        let resolved_items = [
            ConflictedFileItem {
                repo_path: RepoPath::new("src/main.rs").expect("valid repo path"),
                display_name: "main.rs".to_string(),
                relative_path: "src/main.rs".to_string(),
                is_resolved: true,
            },
            ConflictedFileItem {
                repo_path: RepoPath::new("src/lib.rs").expect("valid repo path"),
                display_name: "lib.rs".to_string(),
                relative_path: "src/lib.rs".to_string(),
                is_resolved: true,
            },
        ];
        let all_resolved = !resolved_items.is_empty() && resolved_items.iter().all(|f| f.is_resolved);
        assert!(all_resolved);
    }
}


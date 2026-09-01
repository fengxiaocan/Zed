use editor::{Editor, EditorEvent};
use git::repository::TagInfo;
use gpui::{
    App, ClipboardItem, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, PromptLevel, SharedString, Styled,
    Subscription, Window, rems, uniform_list,
};
use menu::{Cancel, Confirm};
use project::git_store::Repository;
use settings::translate_ui;
use ui::{
    Color, ContextMenu, Headline, HeadlineSize, IconButton, IconName, IconSize, Label, LabelSize,
    PopoverMenu, prelude::*,
};
use workspace::notifications::DetachAndPromptErr;
use workspace::{ModalView, Workspace};

/// Build a single-line filter editor for the Tags section.
pub(crate) fn new_tag_filter_editor(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    super::filter::new_filter_editor_with_placeholder(translate_ui("Filter tags…", cx), window, cx)
}

pub(crate) fn tag_filter_query(editor: &Entity<Editor>, cx: &App) -> String {
    super::filter::filter_query(editor, cx)
}

/// Filter tags by name (case-insensitive substring).
pub(crate) fn filter_tags(tags: &[TagInfo], query: &str) -> Vec<TagInfo> {
    if query.is_empty() {
        return tags.to_vec();
    }
    let query_lower = query.to_lowercase();
    tags.iter()
        .filter(|t| t.name.to_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}

pub(crate) fn render_tag_filter_editor(
    filter_editor: &Entity<Editor>,
    cx: &App,
) -> impl IntoElement {
    super::filter::render_filter_editor(filter_editor, cx)
}

pub(crate) fn render_tag_list(
    tags: Vec<TagInfo>,
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
    if tags.is_empty() {
        return Label::new(translate_ui("No tags", cx))
            .color(Color::Muted)
            .into_any_element();
    }

    let tag_count = tags.len();
    uniform_list("git-manager-tags", tag_count, move |range, _window, cx| {
        tags[range.clone()]
            .iter()
            .enumerate()
            .map(|(offset, tag)| {
                let index = range.start + offset;
                render_tag_row(index, tag.clone(), repo.clone(), workspace.clone(), cx)
            })
            .collect()
    })
    .flex_1()
    .size_full()
    .into_any_element()
}

fn render_tag_row(
    index: usize,
    tag: TagInfo,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
    cx: &App,
) -> AnyElement {
    let id = SharedString::from(format!("gm-tag-row-{index}"));
    let short_sha: String = tag.target.chars().take(8).collect();
    let tag_for_menu = tag.clone();

    h_flex()
        .id(ElementId::Name(id))
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .hover(|s| s.bg(cx.theme().colors().element_hover))
        .child(
            v_flex()
                .flex_1()
                .gap_0()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(Label::new(tag.name.clone()).size(LabelSize::Small))
                        .child(
                            Label::new(short_sha)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        )
                        .when(!tag.is_annotated, |this| {
                            this.child(
                                Label::new(translate_ui("lightweight", cx))
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            )
                        }),
                )
                .when_some(tag.message, |this, message| {
                    this.child(
                        Label::new(message)
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    )
                }),
        )
        .child(
            PopoverMenu::new(format!("gm-tag-menu-{index}"))
                .trigger(
                    IconButton::new(format!("gm-tag-menu-btn-{index}"), IconName::Ellipsis)
                        .icon_size(IconSize::XSmall),
                )
                .menu({
                    let tag = tag_for_menu;
                    let repo = repo;
                    let workspace = workspace;
                    move |window, cx| {
                        let tag = tag.clone();
                        let name = tag.name.clone();
                        let repo = repo.clone();
                        let workspace = workspace.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let checkout_name = name.clone();
                            let checkout_repo = repo.clone();
                            let new_branch_name = name.clone();
                            let new_branch_repo = repo.clone();
                            let new_branch_workspace = workspace.clone();
                            let push_name = name.clone();
                            let push_repo = repo.clone();
                            let push_workspace = workspace.clone();
                            let copy_name = name.clone();
                            let copy_sha = tag.target.clone();

                            menu.entry(translate_ui("Checkout Tag", cx), None, {
                                let name = checkout_name;
                                let repo = checkout_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let name = name.to_string();
                                    let receiver =
                                        repo.update(cx, |repo, _| repo.change_branch(name));
                                    window
                                        .spawn(cx, async move |_cx| {
                                            receiver.await??;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to checkout tag", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .entry(translate_ui("New Branch from Tag…", cx), None, {
                                let name = new_branch_name;
                                let repo = new_branch_repo;
                                let workspace = new_branch_workspace;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let name = name.to_string();
                                    if let Some(workspace) = workspace.upgrade() {
                                        workspace.update(cx, |workspace, cx| {
                                            let title = format!(
                                                "{} ({})",
                                                translate_ui("New Branch from Tag", cx),
                                                name
                                            );
                                            workspace.toggle_modal(window, cx, |window, cx| {
                                                crate::NewBranchModal::new(
                                                    title,
                                                    name.clone(),
                                                    Some(format!("refs/tags/{name}")),
                                                    repo,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        });
                                    }
                                }
                            })
                            .entry(translate_ui("Push Tag to Remote", cx), None, {
                                let name = push_name;
                                let repo = push_repo;
                                let workspace = push_workspace;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let tag_name = name.to_string();
                                    let askpass = crate::git_manager::operations::askpass_delegate(
                                        &workspace,
                                        format!("git push origin {tag_name}"),
                                        window,
                                        cx,
                                    );
                                    let receiver = repo.update(cx, |repo, cx| {
                                        repo.push(
                                            format!("refs/tags/{tag_name}").into(),
                                            format!("refs/tags/{tag_name}").into(),
                                            "origin".into(),
                                            None,
                                            askpass,
                                            cx,
                                        )
                                    });
                                    window
                                        .spawn(cx, async move |_cx| {
                                            receiver.await??;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to push tag", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .separator()
                            .entry(translate_ui("Copy Tag Name", cx), None, {
                                let name = copy_name;
                                move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        name.to_string(),
                                    ));
                                }
                            })
                            .entry(translate_ui("Copy Commit SHA", cx), None, {
                                let sha = copy_sha;
                                move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        sha.to_string(),
                                    ));
                                }
                            })
                            .separator()
                            .entry(
                                translate_ui("Delete Tag", cx),
                                None,
                                {
                                    let name = name;
                                    let repo = repo;
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        let name = name.to_string();
                                        let prompt_message = format!(
                                            "{} '{}'?",
                                            translate_ui("Delete tag", cx),
                                            name
                                        );
                                        let buttons = [
                                            translate_ui("Delete", cx),
                                            translate_ui("Cancel", cx),
                                        ];
                                        let answer = window.prompt(
                                            PromptLevel::Warning,
                                            &prompt_message,
                                            None,
                                            &buttons,
                                            cx,
                                        );
                                        window
                                            .spawn(cx, async move |cx| {
                                                if answer.await != Ok(0) {
                                                    return anyhow::Ok(());
                                                }
                                                repo.update(cx, |repo, _| {
                                                    repo.delete_tag(name.clone())
                                                })
                                                .await??;
                                                anyhow::Ok(())
                                            })
                                            .detach_and_prompt_err(
                                                translate_ui("Failed to delete tag", cx),
                                                window,
                                                cx,
                                                |e, _, _| Some(e.to_string()),
                                            );
                                    }
                                },
                            )
                        }))
                    }
                }),
        )
        .into_any_element()
}

/// Modal to create a new tag (name + optional annotated message).
pub(crate) struct NewTagModal {
    name_editor: Entity<Editor>,
    message_editor: Entity<Editor>,
    repo: Entity<Repository>,
    _name_subscription: Subscription,
    _message_subscription: Subscription,
}

impl NewTagModal {
    pub(crate) fn new(
        repo: Entity<Repository>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_placeholder = translate_ui("Tag name…", cx);
        let message_placeholder = translate_ui("Message (optional, creates annotated tag)…", cx);

        let name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(name_placeholder, window, cx);
            editor
        });
        let message_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(message_placeholder, window, cx);
            editor
        });

        let _name_subscription = cx.subscribe(&name_editor, |_, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::BufferEdited) {
                cx.notify();
            }
        });
        let _message_subscription =
            cx.subscribe(&message_editor, |_, _, event: &EditorEvent, cx| {
                if matches!(event, EditorEvent::BufferEdited) {
                    cx.notify();
                }
            });

        Self {
            name_editor,
            message_editor,
            repo,
            _name_subscription,
            _message_subscription,
        }
    }

    fn cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_editor.read(cx).text(cx).trim().to_string();
        let message = self.message_editor.read(cx).text(cx).trim().to_string();
        if name.is_empty() {
            return;
        }
        let message = if message.is_empty() {
            None
        } else {
            Some(message)
        };

        let repo = self.repo.clone();
        cx.spawn(async move |_, cx| {
            repo.update(cx, |repo, _| repo.create_tag(name, None, message))
                .await??;
            anyhow::Ok(())
        })
        .detach_and_prompt_err(
            translate_ui("Failed to create tag", cx),
            window,
            cx,
            |e, _, _| Some(e.to_string()),
        );
        cx.emit(DismissEvent);
    }
}

impl EventEmitter<DismissEvent> for NewTagModal {}
impl ModalView for NewTagModal {}
impl Focusable for NewTagModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name_editor.focus_handle(cx)
    }
}

impl Render for NewTagModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context("GitManagerNewTagModal")
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::confirm))
            .elevation_2(cx)
            .w(rems(34.))
            .child(
                h_flex()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .w_full()
                    .gap_1p5()
                    .child(Icon::new(IconName::Hash).size(IconSize::XSmall))
                    .child(Headline::new(translate_ui("New Tag", cx)).size(HeadlineSize::XSmall)),
            )
            .child(
                v_flex()
                    .px_3()
                    .pb_3()
                    .w_full()
                    .gap_2()
                    .child(self.name_editor.clone())
                    .child(self.message_editor.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::filter_tags;
    use git::repository::TagInfo;
    use gpui::SharedString;

    fn tag(name: &str, annotated: bool) -> TagInfo {
        TagInfo {
            name: SharedString::from(name),
            target: SharedString::from("a".repeat(40)),
            message: annotated.then(|| SharedString::from("msg")),
            is_annotated: annotated,
        }
    }

    #[test]
    fn filter_tags_matches_name() {
        let tags = vec![tag("v1.0", true), tag("release-2", false)];
        assert_eq!(filter_tags(&tags, "").len(), 2);
        let filtered = filter_tags(&tags, "v1");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name.as_ref(), "v1.0");
    }
}

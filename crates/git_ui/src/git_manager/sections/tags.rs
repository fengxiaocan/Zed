use editor::{Editor, EditorElement, EditorEvent, EditorStyle};
use git::repository::TagInfo;
use gpui::{
    App, ClipboardItem, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Subscription, TextStyle,
    Window, div, relative, rems, uniform_list,
};
use menu::{Cancel, Confirm};
use project::git_store::Repository;
use settings::{Settings, translate_ui};
use theme_settings::ThemeSettings;
use ui::{
    Color, ContextMenu, Headline, HeadlineSize, IconButton, IconName, IconSize, Label, LabelSize,
    PopoverMenu, prelude::*,
};
use workspace::notifications::DetachAndPromptErr;
use workspace::{ModalView, Workspace};

/// Build a single-line filter editor for the Tags section.
pub(crate) fn new_tag_filter_editor(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    cx.new(|cx| {
        let mut editor = Editor::single_line(window, cx);
        editor.set_placeholder_text(translate_ui("Filter tags…", cx), window, cx);
        editor
    })
}

pub(crate) fn tag_filter_query(editor: &Entity<Editor>, cx: &App) -> String {
    editor.read(cx).text(cx)
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
    let settings = ThemeSettings::get_global(cx);
    let text_style = TextStyle {
        color: cx.theme().colors().text,
        font_family: settings.ui_font.family.clone(),
        font_features: settings.ui_font.features.clone(),
        font_fallbacks: settings.ui_font.fallbacks.clone(),
        font_size: rems(0.875).into(),
        font_weight: settings.ui_font.weight,
        line_height: relative(1.3),
        ..Default::default()
    };

    h_flex()
        .w_full()
        .h_8()
        .px_1p5()
        .gap_2()
        .border_1()
        .border_color(cx.theme().colors().border)
        .rounded_md()
        .bg(cx.theme().colors().editor_background)
        .child(Icon::new(IconName::MagnifyingGlass).color(Color::Muted))
        .child(div().flex_1().child(EditorElement::new(
            filter_editor,
            EditorStyle {
                background: cx.theme().colors().editor_background,
                local_player: cx.theme().players().local(),
                text: text_style,
                ..Default::default()
            },
        )))
}

pub(crate) fn render_tag_list(
    tags: Vec<TagInfo>,
    has_repo: bool,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
    cx: &App,
) -> AnyElement {
    if !has_repo {
        return Label::new(translate_ui("No repository found", cx))
            .color(Color::Muted)
            .into_any_element();
    }
    if tags.is_empty() {
        return Label::new(translate_ui("No tags", cx))
            .color(Color::Muted)
            .into_any_element();
    }

    let tag_count = tags.len();
    uniform_list(
        "git-manager-tags",
        tag_count,
        move |range, _window, cx| {
            tags[range.clone()]
                .iter()
                .enumerate()
                .map(|(offset, tag)| {
                    let index = range.start + offset;
                    render_tag_row(index, tag.clone(), repo.clone(), workspace.clone(), cx)
                })
                .collect()
        },
    )
    .flex_1()
    .size_full()
    .into_any_element()
}

fn render_tag_row(
    index: usize,
    tag: TagInfo,
    repo: Option<Entity<Repository>>,
    _workspace: gpui::WeakEntity<Workspace>,
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
                .when_some(tag.message.clone(), |this, message| {
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
                    move |window, cx| {
                        let tag = tag.clone();
                        let name = tag.name.clone();
                        let repo = repo.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let copy_name = name.clone();
                            let copy_sha = tag.target.clone();
                            let delete_name = name.clone();
                            let delete_repo = repo.clone();

                            menu.entry(translate_ui("Copy Tag Name", cx), None, {
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
                            .entry(translate_ui("Delete Tag", cx), None, {
                                let name = delete_name;
                                let repo = delete_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let name = name.to_string();
                                    window
                                        .spawn(cx, async move |cx| {
                                            repo.update(cx, |repo, _| {
                                                repo.delete_tag(name.clone())
                                            })
                                            .await??;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            "Failed to delete tag",
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
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
    pub(crate) fn new(repo: Entity<Repository>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        .detach_and_prompt_err("Failed to create tag", window, cx, |e, _, _| {
            Some(e.to_string())
        });
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
                    .child(
                        Headline::new(translate_ui("New Tag", cx)).size(HeadlineSize::XSmall),
                    ),
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

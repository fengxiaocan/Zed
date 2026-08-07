use editor::{Editor, EditorElement, EditorStyle};
use git::stash::StashEntry;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, PromptLevel, SharedString, Styled,
    TextStyle, Window, div, relative, rems, uniform_list,
};
use project::git_store::Repository;
use settings::{Settings, translate_ui};
use theme_settings::ThemeSettings;
use ui::{
    Color, ContextMenu, IconButton, IconName, IconSize, Label, LabelSize, PopoverMenu, prelude::*,
};
use workspace::Workspace;
use workspace::notifications::DetachAndPromptErr;

/// Build a single-line filter editor for the Shelves section.
pub(crate) fn new_shelf_filter_editor(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    cx.new(|cx| {
        let mut editor = Editor::single_line(window, cx);
        editor.set_placeholder_text(translate_ui("Filter shelves…", cx), window, cx);
        editor
    })
}

pub(crate) fn shelf_filter_query(editor: &Entity<Editor>, cx: &App) -> String {
    editor.read(cx).text(cx)
}

/// Filter stash entries by message or branch (case-insensitive substring).
pub(crate) fn filter_shelves(entries: &[StashEntry], query: &str) -> Vec<StashEntry> {
    if query.is_empty() {
        return entries.to_vec();
    }
    let query_lower = query.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            e.message.to_lowercase().contains(&query_lower)
                || e.branch
                    .as_deref()
                    .is_some_and(|b| b.to_lowercase().contains(&query_lower))
        })
        .cloned()
        .collect()
}

pub(crate) fn render_shelf_filter_editor(
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

pub(crate) fn render_shelf_list(
    entries: Vec<StashEntry>,
    has_repo: bool,
    repo: Option<Entity<Repository>>,
    _workspace: gpui::WeakEntity<Workspace>,
    cx: &App,
) -> AnyElement {
    if !has_repo {
        return Label::new(translate_ui("No repository found", cx))
            .color(Color::Muted)
            .into_any_element();
    }
    if entries.is_empty() {
        return Label::new(translate_ui("No shelves", cx))
            .color(Color::Muted)
            .into_any_element();
    }

    let entry_count = entries.len();
    uniform_list(
        "git-manager-shelves",
        entry_count,
        move |range, _window, cx| {
            entries[range.clone()]
                .iter()
                .enumerate()
                .map(|(offset, entry)| {
                    let index = range.start + offset;
                    render_shelf_row(index, entry.clone(), repo.clone(), cx)
                })
                .collect()
        },
    )
    .flex_1()
    .size_full()
    .into_any_element()
}

fn render_shelf_row(
    index: usize,
    entry: StashEntry,
    repo: Option<Entity<Repository>>,
    cx: &App,
) -> AnyElement {
    let id = SharedString::from(format!("gm-shelf-row-{index}"));
    let short_sha: String = entry.oid.to_string().chars().take(8).collect();
    let branch = entry.branch.clone().unwrap_or_default();
    let entry_for_menu = entry.clone();

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
                        .child(
                            Label::new(format!("stash@{{{}}}", entry.index))
                                .size(LabelSize::Small),
                        )
                        .child(
                            Label::new(short_sha)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        )
                        .when(!branch.is_empty(), |this| {
                            this.child(
                                Label::new(branch)
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            )
                        }),
                )
                .child(
                    Label::new(entry.message.clone())
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                ),
        )
        .child(
            PopoverMenu::new(format!("gm-shelf-menu-{index}"))
                .trigger(
                    IconButton::new(format!("gm-shelf-menu-btn-{index}"), IconName::Ellipsis)
                        .icon_size(IconSize::XSmall),
                )
                .menu({
                    let entry = entry_for_menu;
                    let repo = repo;
                    move |window, cx| {
                        let entry = entry.clone();
                        let repo = repo.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let apply_repo = repo.clone();
                            let pop_repo = repo.clone();
                            let drop_repo = repo.clone();
                            let apply_index = entry.index;
                            let pop_index = entry.index;
                            let drop_index = entry.index;

                            menu.entry(translate_ui("Apply Shelf", cx), None, {
                                let repo = apply_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    window
                                        .spawn(cx, async move |cx| {
                                            let task = repo.update(cx, |repo, cx| {
                                                repo.stash_apply(Some(apply_index), cx)
                                            });
                                            task.await?;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to apply shelf", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .entry(translate_ui("Pop Shelf", cx), None, {
                                let repo = pop_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    window
                                        .spawn(cx, async move |cx| {
                                            let task = repo.update(cx, |repo, cx| {
                                                repo.stash_pop(Some(pop_index), cx)
                                            });
                                            task.await?;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to pop shelf", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .separator()
                            .entry(translate_ui("Drop Shelf", cx), None, {
                                let repo = drop_repo;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let prompt_message = format!(
                                        "{} stash@{{{}}}?",
                                        translate_ui("Drop shelf", cx),
                                        drop_index
                                    );
                                    let buttons =
                                        [translate_ui("Drop", cx), translate_ui("Cancel", cx)];
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
                                            repo.update(cx, |repo, cx| {
                                                repo.stash_drop(Some(drop_index), cx)
                                            })
                                            .await??;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to drop shelf", cx),
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

#[cfg(test)]
mod tests {
    use super::filter_shelves;
    use git::Oid;
    use git::stash::StashEntry;
    use std::str::FromStr;

    fn entry(index: usize, branch: Option<&str>, message: &str) -> StashEntry {
        StashEntry {
            index,
            oid: Oid::from_str(&"a".repeat(40)).unwrap(),
            message: message.to_string(),
            branch: branch.map(Into::into),
            timestamp: 0,
        }
    }

    #[test]
    fn filter_shelves_matches_message_or_branch() {
        let entries = vec![
            entry(0, Some("main"), "wip on feature"),
            entry(1, Some("develop"), "stash work"),
        ];
        assert_eq!(filter_shelves(&entries, "").len(), 2);
        let by_message = filter_shelves(&entries, "wip");
        assert_eq!(by_message.len(), 1);
        assert_eq!(by_message[0].index, 0);
        let by_branch = filter_shelves(&entries, "develop");
        assert_eq!(by_branch.len(), 1);
        assert_eq!(by_branch[0].index, 1);
    }
}

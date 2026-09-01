use editor::{Editor, EditorEvent};
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

/// (name, url) for one remote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteEntry {
    pub name: SharedString,
    pub url: SharedString,
}

/// Sort remote entries by name for stable display.
pub(crate) fn sort_remotes(remotes: &mut [RemoteEntry]) {
    remotes.sort_by(|a, b| a.name.cmp(&b.name));
}

/// Filter remotes by name or URL (case-insensitive substring).
pub(crate) fn filter_remotes(remotes: &[RemoteEntry], query: &str) -> Vec<RemoteEntry> {
    if query.is_empty() {
        return remotes.to_vec();
    }
    let query_lower = query.to_lowercase();
    remotes
        .iter()
        .filter(|r| {
            r.name.to_lowercase().contains(&query_lower)
                || r.url.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect()
}

/// Build a single-line filter editor for the Remotes section.
pub(crate) fn new_remote_filter_editor(window: &mut Window, cx: &mut App) -> Entity<Editor> {
    super::filter::new_filter_editor_with_placeholder(
        translate_ui("Filter remotes…", cx),
        window,
        cx,
    )
}

pub(crate) fn remote_filter_query(editor: &Entity<Editor>, cx: &App) -> String {
    super::filter::filter_query(editor, cx)
}

pub(crate) fn render_remote_filter_editor(
    filter_editor: &Entity<Editor>,
    cx: &App,
) -> impl IntoElement {
    super::filter::render_filter_editor(filter_editor, cx)
}

pub(crate) fn render_remote_list(
    remotes: Vec<RemoteEntry>,
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
    if remotes.is_empty() {
        return Label::new(translate_ui("No remotes", cx))
            .color(Color::Muted)
            .into_any_element();
    }

    let remote_count = remotes.len();
    uniform_list(
        "git-manager-remotes",
        remote_count,
        move |range, _window, cx| {
            remotes[range.clone()]
                .iter()
                .enumerate()
                .map(|(offset, remote)| {
                    let index = range.start + offset;
                    render_remote_row(index, remote.clone(), repo.clone(), workspace.clone(), cx)
                })
                .collect()
        },
    )
    .flex_1()
    .size_full()
    .into_any_element()
}

fn render_remote_row(
    index: usize,
    remote: RemoteEntry,
    repo: Option<Entity<Repository>>,
    workspace: gpui::WeakEntity<Workspace>,
    cx: &App,
) -> AnyElement {
    let id = SharedString::from(format!("gm-remote-row-{index}"));
    let remote_for_menu = remote.clone();

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
                .child(Label::new(remote.name.clone()).size(LabelSize::Small))
                .child(
                    Label::new(remote.url)
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                ),
        )
        .child(
            PopoverMenu::new(format!("gm-remote-menu-{index}"))
                .trigger(
                    IconButton::new(format!("gm-remote-menu-btn-{index}"), IconName::Ellipsis)
                        .icon_size(IconSize::XSmall),
                )
                .menu({
                    let remote = remote_for_menu;
                    let repo = repo;
                    let workspace = workspace;
                    move |window, cx| {
                        let name = remote.name.clone();
                        let url = remote.url.clone();
                        let repo = repo.clone();
                        let workspace = workspace.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            let copy_name = name.clone();
                            let copy_url = url.clone();
                            let fetch_name = name.clone();
                            let fetch_repo = repo.clone();
                            let fetch_workspace = workspace.clone();
                            let prune_name = name.clone();
                            let prune_repo = repo.clone();
                            let prune_workspace = workspace.clone();
                            let edit_name = name.clone();
                            let edit_url = url.clone();
                            let edit_repo = repo.clone();
                            let edit_workspace = workspace.clone();
                            let remove_name = name.clone();
                            let remove_repo = repo.clone();
                            let remove_workspace = workspace;

                            menu.entry(translate_ui("Fetch Remote", cx), None, {
                                let name = fetch_name;
                                let repo = fetch_repo;
                                let workspace = fetch_workspace;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let name_str = name.to_string();
                                    let askpass = crate::git_manager::operations::askpass_delegate(
                                        &workspace,
                                        format!("git fetch {name_str}"),
                                        window,
                                        cx,
                                    );
                                    let remote_obj = git::repository::Remote { name: name.clone() };
                                    let receiver = repo.update(cx, |repo, cx| {
                                        repo.fetch(
                                            git::repository::FetchOptions::Remote(remote_obj),
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
                                            translate_ui("Failed to fetch remote", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .entry(translate_ui("Prune Remote Branches", cx), None, {
                                let name = prune_name;
                                let repo = prune_repo;
                                let workspace = prune_workspace;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let name_str = name.to_string();
                                    let askpass = crate::git_manager::operations::askpass_delegate(
                                        &workspace,
                                        format!("git remote prune {name_str}"),
                                        window,
                                        cx,
                                    );
                                    let receiver = repo.update(cx, |repo, cx| {
                                        repo.prune_remote(name_str, askpass, cx)
                                    });
                                    window
                                        .spawn(cx, async move |_cx| {
                                            receiver.await??;
                                            anyhow::Ok(())
                                        })
                                        .detach_and_prompt_err(
                                            translate_ui("Failed to prune remote", cx),
                                            window,
                                            cx,
                                            |e, _, _| Some(e.to_string()),
                                        );
                                }
                            })
                            .separator()
                            .entry(translate_ui("Copy Name", cx), None, {
                                let name = copy_name;
                                move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        name.to_string(),
                                    ));
                                }
                            })
                            .entry(translate_ui("Copy URL", cx), None, {
                                let url = copy_url;
                                move |_, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        url.to_string(),
                                    ));
                                }
                            })
                            .separator()
                            .entry(translate_ui("Edit URL…", cx), None, {
                                let name = edit_name;
                                let url = edit_url;
                                let repo = edit_repo;
                                let workspace = edit_workspace;
                                move |window, cx| {
                                    let Some(repo) = repo.clone() else {
                                        return;
                                    };
                                    let Some(workspace) = workspace.upgrade() else {
                                        return;
                                    };
                                    let name = name.to_string();
                                    let url = url.to_string();
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.toggle_modal(window, cx, |window, cx| {
                                            RemoteModal::new_edit(name, url, repo, window, cx)
                                        });
                                    });
                                }
                            })
                            .entry(
                                translate_ui("Remove", cx),
                                None,
                                {
                                    let name = remove_name;
                                    let repo = remove_repo;
                                    let workspace = remove_workspace;
                                    move |window, cx| {
                                        let Some(repo) = repo.clone() else {
                                            return;
                                        };
                                        if workspace.upgrade().is_none() {
                                            return;
                                        }
                                        let name = name.to_string();
                                        let prompt_message = format!(
                                            "{} '{}'?",
                                            translate_ui("Remove remote", cx),
                                            name
                                        );
                                        let buttons = [
                                            translate_ui("Remove", cx),
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
                                                    repo.remove_remote(name.clone())
                                                })
                                                .await??;
                                                anyhow::Ok(())
                                            })
                                            .detach_and_prompt_err(
                                                translate_ui("Failed to remove remote", cx),
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

/// Modal to add or edit a remote (name + URL).
pub(crate) struct RemoteModal {
    name_editor: Entity<Editor>,
    url_editor: Entity<Editor>,
    repo: Entity<Repository>,
    /// `Some(name)` when editing an existing remote; `None` when adding.
    editing_name: Option<SharedString>,
    _name_subscription: Subscription,
    _url_subscription: Subscription,
}

impl RemoteModal {
    pub(crate) fn new_add(
        repo: Entity<Repository>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(None, String::new(), String::new(), repo, window, cx)
    }

    pub(crate) fn new_edit(
        name: String,
        url: String,
        repo: Entity<Repository>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(Some(name.clone().into()), name, url, repo, window, cx)
    }

    fn new(
        editing_name: Option<SharedString>,
        initial_name: String,
        initial_url: String,
        repo: Entity<Repository>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_placeholder = translate_ui("Remote name…", cx);
        let url_placeholder = translate_ui("Remote URL…", cx);

        let name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(name_placeholder, window, cx);
            editor.set_text(initial_name, window, cx);
            editor
        });
        let url_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text(url_placeholder, window, cx);
            editor.set_text(initial_url, window, cx);
            editor
        });

        let _name_subscription = cx.subscribe(&name_editor, |_, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::BufferEdited) {
                cx.notify();
            }
        });
        let _url_subscription = cx.subscribe(&url_editor, |_, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::BufferEdited) {
                cx.notify();
            }
        });

        Self {
            name_editor,
            url_editor,
            repo,
            editing_name,
            _name_subscription,
            _url_subscription,
        }
    }

    fn cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_editor.read(cx).text(cx).trim().to_string();
        let url = self.url_editor.read(cx).text(cx).trim().to_string();
        if name.is_empty() || url.is_empty() {
            return;
        }

        let repo = self.repo.clone();
        let editing_name = self.editing_name.clone();

        if let Some(old_name) = editing_name {
            // Editing URL: remove + re-add with same name (set_remote_url not on trait yet).
            let old_name = old_name.to_string();
            cx.spawn(async move |_, cx| {
                repo.update(cx, |repo, _| repo.remove_remote(old_name))
                    .await??;
                repo.update(cx, |repo, _| repo.create_remote(name, url))
                    .await??;
                anyhow::Ok(())
            })
            .detach_and_prompt_err(
                translate_ui("Failed to update remote URL", cx),
                window,
                cx,
                |e, _, _| Some(e.to_string()),
            );
        } else {
            cx.spawn(async move |_, cx| {
                repo.update(cx, |repo, _| repo.create_remote(name, url))
                    .await??;
                anyhow::Ok(())
            })
            .detach_and_prompt_err(
                translate_ui("Failed to add remote", cx),
                window,
                cx,
                |e, _, _| Some(e.to_string()),
            );
        }
        cx.emit(DismissEvent);
    }
}

impl EventEmitter<DismissEvent> for RemoteModal {}
impl ModalView for RemoteModal {}
impl Focusable for RemoteModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name_editor.focus_handle(cx)
    }
}

impl Render for RemoteModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.editing_name {
            Some(name) => format!("{} ({})", translate_ui("Edit Remote", cx), name),
            None => translate_ui("Add Remote", cx).to_string(),
        };

        v_flex()
            .key_context("GitManagerRemoteModal")
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
                    .child(Icon::new(IconName::ArrowCircle).size(IconSize::XSmall))
                    .child(Headline::new(title).size(HeadlineSize::XSmall)),
            )
            .child(
                v_flex()
                    .px_3()
                    .pb_3()
                    .w_full()
                    .gap_2()
                    .child(self.name_editor.clone())
                    .child(self.url_editor.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{RemoteEntry, filter_remotes, sort_remotes};
    use gpui::SharedString;

    fn remote(name: &str, url: &str) -> RemoteEntry {
        RemoteEntry {
            name: SharedString::from(name),
            url: SharedString::from(url),
        }
    }

    #[test]
    fn sort_remotes_by_name() {
        let mut remotes = vec![remote("zeta", "u1"), remote("alpha", "u2")];
        sort_remotes(&mut remotes);
        assert_eq!(remotes[0].name.as_ref(), "alpha");
        assert_eq!(remotes[1].name.as_ref(), "zeta");
    }

    #[test]
    fn filter_remotes_matches_name_or_url() {
        let remotes = vec![
            remote("origin", "https://github.com/x/y.git"),
            remote("upstream", "git@gitlab.com:a/b.git"),
        ];
        assert_eq!(filter_remotes(&remotes, "").len(), 2);
        let by_name = filter_remotes(&remotes, "origin");
        assert_eq!(by_name.len(), 1);
        let by_url = filter_remotes(&remotes, "gitlab");
        assert_eq!(by_url.len(), 1);
        assert_eq!(by_url[0].name.as_ref(), "upstream");
    }
}

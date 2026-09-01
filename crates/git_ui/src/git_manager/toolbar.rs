use crate::git_manager::GitManagerTab;
use gpui::{
    Action, App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    Window, div,
};
use settings::translate_ui;
use ui::{
    Button, ButtonSize, ContextMenu, IconButton, IconName, IconSize, PopoverMenu, Tab, Tooltip,
    prelude::*,
};

/// Primary + overflow toolbar for the Git Manager panel (P0 skeleton).
#[derive(IntoElement)]
pub(crate) struct GitManagerToolbar {
    focus_handle: FocusHandle,
    manager: gpui::WeakEntity<crate::git_manager::GitManager>,
}

impl GitManagerToolbar {
    pub(crate) fn new(
        focus_handle: FocusHandle,
        manager: gpui::WeakEntity<crate::git_manager::GitManager>,
    ) -> Self {
        Self {
            focus_handle,
            manager,
        }
    }
}

impl RenderOnce for GitManagerToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = self.focus_handle;
        let manager_fetch_all = self.manager.clone();
        let manager_tag = self.manager.clone();
        let manager_shelf = self.manager.clone();
        let manager_update = self.manager;

        h_flex()
            .w_full()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(toolbar_action_button(
                "gm-checkout",
                translate_ui("Checkout", cx),
                zed_actions::git::CheckoutBranch.boxed_clone(),
                focus_handle.clone(),
            ))
            .child(toolbar_action_button(
                "gm-branch",
                translate_ui("Branch", cx),
                zed_actions::git::Branch.boxed_clone(),
                focus_handle.clone(),
            ))
            .child(toolbar_action_button(
                "gm-pull",
                translate_ui("Pull", cx),
                git::Pull.boxed_clone(),
                focus_handle.clone(),
            ))
            .child(toolbar_action_button(
                "gm-push",
                translate_ui("Push", cx),
                git::Push.boxed_clone(),
                focus_handle.clone(),
            ))
            .child(toolbar_action_button(
                "gm-fetch",
                translate_ui("Fetch", cx),
                git::Fetch.boxed_clone(),
                focus_handle,
            ))
            .child(
                Button::new("gm-update-project", translate_ui("Update Project", cx))
                    .label_size(LabelSize::Small)
                    .size(ButtonSize::Compact)
                    .tooltip(Tooltip::text(translate_ui(
                        "Fetch and integrate changes from upstream",
                        cx,
                    )))
                    .on_click({
                        move |_, window, cx| {
                            if let Some(manager) = manager_update.upgrade() {
                                manager.update(cx, |manager, cx| {
                                    manager.update_project(window, cx);
                                });
                            }
                        }
                    }),
            )
            .child(div().flex_1())
            .child(
                PopoverMenu::new("gm-overflow-menu")
                    .trigger_with_tooltip(
                        IconButton::new("gm-overflow-trigger", IconName::Ellipsis)
                            .icon_size(IconSize::Small),
                        Tooltip::text(translate_ui("More", cx)),
                    )
                    .menu(move |window, cx| {
                        let manager_fetch_all = manager_fetch_all.clone();
                        let manager_tag = manager_tag.clone();
                        let manager_shelf = manager_shelf.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            menu.action(translate_ui("Fetch", cx), git::Fetch.boxed_clone())
                                .entry(
                                    translate_ui("Fetch All Remotes", cx),
                                    None,
                                    {
                                        let manager = manager_fetch_all;
                                        move |window, cx| {
                                            if let Some(manager) = manager.upgrade() {
                                                manager.update(cx, |manager, cx| {
                                                    manager.fetch_all_remotes(window, cx);
                                                });
                                            }
                                        }
                                    },
                                )
                                .action(
                                    translate_ui("Open Git Panel", cx),
                                    zed_actions::git_panel::ToggleFocus.boxed_clone(),
                                )
                                .action(
                                    translate_ui("Open Git Graph", cx),
                                    crate::git_graph::Open.boxed_clone(),
                                )
                                .separator()
                                .entry(
                                    translate_ui("New Tag", cx),
                                    None,
                                    {
                                        let manager = manager_tag;
                                        move |_, cx| {
                                            if let Some(manager) = manager.upgrade() {
                                                manager.update(cx, |manager, cx| {
                                                    manager.set_active_tab(
                                                        crate::git_manager::GitManagerTab::Tags,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }
                                    },
                                )
                                .entry(
                                    translate_ui("Shelf", cx),
                                    None,
                                    {
                                        let manager = manager_shelf;
                                        move |_, cx| {
                                            if let Some(manager) = manager.upgrade() {
                                                manager.update(cx, |manager, cx| {
                                                    manager.set_active_tab(
                                                        crate::git_manager::GitManagerTab::Shelves,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }
                                    },
                                )
                        }))
                    }),
            )
    }
}

fn toolbar_action_button(
    id: impl Into<SharedString>,
    label: &'static str,
    action: Box<dyn Action>,
    focus_handle: FocusHandle,
) -> Button {
    let id = id.into();
    let action_for_click = action.boxed_clone();
    Button::new(id, label)
        .label_size(LabelSize::Small)
        .size(ButtonSize::Compact)
        .tooltip({
            let label = SharedString::from(label);
            move |_, cx| {
                Tooltip::for_action_in(label.clone(), action.as_ref(), &focus_handle, cx)
            }
        })
        .on_click(move |_, window, cx| {
            window.dispatch_action(action_for_click.boxed_clone(), cx);
        })
}

/// Tab strip under the header (Branches / Remotes / Tags / Shelves).
pub(crate) fn render_tab_bar(
    active_tab: GitManagerTab,
    branches_count: usize,
    remotes_count: usize,
    tags_count: usize,
    shelves_count: usize,
    cx: &mut Context<crate::git_manager::GitManager>,
) -> impl IntoElement {
    let make_tab = |id: SharedString,
                    label: &'static str,
                    count: usize,
                    tab: GitManagerTab,
                    active: bool,
                    cx: &mut Context<crate::git_manager::GitManager>| {
        let localized = translate_ui(label, cx);
        let display_label = if count > 0 {
            format!("{localized} ({count})")
        } else {
            localized.to_string()
        };
        h_flex()
            .id(ElementId::Name(id))
            .cursor_pointer()
            .h_full()
            .px_2()
            .py_1()
            .flex_1()
            .justify_center()
            .hover(|s| s.bg(cx.theme().colors().element_hover))
            .border_b_1()
            .when(!active, |s| {
                s.bg(cx.theme().colors().editor_background.opacity(0.6))
                    .border_color(cx.theme().colors().border.opacity(0.6))
            })
            .when(active, |s| s.border_color(cx.theme().colors().border))
            .child(
                Label::new(display_label)
                    .size(LabelSize::Small)
                    .when(!active, |this| this.color(Color::Muted)),
            )
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.set_active_tab(tab, cx);
            }))
    };

    h_flex()
        .w_full()
        .h(Tab::container_height(cx))
        .border_b_1()
        .border_color(cx.theme().colors().border)
        .child(make_tab(
            "gm-tab-branches".into(),
            "Branches",
            branches_count,
            GitManagerTab::Branches,
            active_tab == GitManagerTab::Branches,
            cx,
        ))
        .child(make_tab(
            "gm-tab-remotes".into(),
            "Remotes",
            remotes_count,
            GitManagerTab::Remotes,
            active_tab == GitManagerTab::Remotes,
            cx,
        ))
        .child(make_tab(
            "gm-tab-tags".into(),
            "Tags",
            tags_count,
            GitManagerTab::Tags,
            active_tab == GitManagerTab::Tags,
            cx,
        ))
        .child(make_tab(
            "gm-tab-shelves".into(),
            "Shelves",
            shelves_count,
            GitManagerTab::Shelves,
            active_tab == GitManagerTab::Shelves,
            cx,
        ))
}

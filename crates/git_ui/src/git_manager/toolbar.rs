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
        let focus_handle = self.focus_handle.clone();
        let manager = self.manager.clone();
        let manager_tag = manager.clone();
        let manager_shelf = manager.clone();
        let manager_update = manager.clone();

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
                focus_handle.clone(),
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
                        let manager_update = manager_update.clone();
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
                        let manager_tag = manager_tag.clone();
                        let manager_shelf = manager_shelf.clone();
                        Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                            menu.action(translate_ui("Fetch", cx), git::Fetch.boxed_clone())
                                .action(
                                    translate_ui("Open Git Panel", cx),
                                    zed_actions::git_panel::ToggleFocus.boxed_clone(),
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
            let focus_handle = focus_handle.clone();
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
    cx: &mut Context<crate::git_manager::GitManager>,
) -> impl IntoElement {
    let make_tab = |id: SharedString,
                    label: SharedString,
                    tab: GitManagerTab,
                    active: bool,
                    cx: &mut Context<crate::git_manager::GitManager>| {
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
            .child(Label::new(label).when(!active, |this| this.color(Color::Muted)))
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
            translate_ui("Branches", cx).into(),
            GitManagerTab::Branches,
            active_tab == GitManagerTab::Branches,
            cx,
        ))
        .child(make_tab(
            "gm-tab-remotes".into(),
            translate_ui("Remotes", cx).into(),
            GitManagerTab::Remotes,
            active_tab == GitManagerTab::Remotes,
            cx,
        ))
        .child(make_tab(
            "gm-tab-tags".into(),
            translate_ui("Tags", cx).into(),
            GitManagerTab::Tags,
            active_tab == GitManagerTab::Tags,
            cx,
        ))
        .child(make_tab(
            "gm-tab-shelves".into(),
            translate_ui("Shelves", cx).into(),
            GitManagerTab::Shelves,
            active_tab == GitManagerTab::Shelves,
            cx,
        ))
}

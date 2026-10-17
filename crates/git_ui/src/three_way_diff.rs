use editor::Editor;
use gpui::{
    actions, App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, FontWeight,
    KeyContext, Render, ScrollHandle, WeakEntity, Window, rems,
};
use project::ConflictRegion;
use settings::translate_ui;
use ui::prelude::*;
use workspace::{ModalView, Workspace};

actions!(three_way_diff, [Cancel, Confirm]);

pub struct ThreeWayConflictModal {
    conflict: ConflictRegion,
    editor: WeakEntity<Editor>,
    file_path: Option<String>,
    ours_lines: Vec<String>,
    theirs_lines: Vec<String>,
    base_lines: Option<Vec<String>>,
    focus_handle: FocusHandle,
    ours_scroll_handle: ScrollHandle,
    theirs_scroll_handle: ScrollHandle,
    base_scroll_handle: ScrollHandle,
}

impl ThreeWayConflictModal {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        workspace: &mut Workspace,
        conflict: ConflictRegion,
        editor: WeakEntity<Editor>,
        ours_text: String,
        theirs_text: String,
        base_text: Option<String>,
        file_path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        workspace.toggle_modal(window, cx, move |_window, cx| {
            Self::new(
                conflict,
                editor,
                ours_text,
                theirs_text,
                base_text,
                file_path,
                cx,
            )
        });
    }

    pub fn new(
        conflict: ConflictRegion,
        editor: WeakEntity<Editor>,
        ours_text: String,
        theirs_text: String,
        base_text: Option<String>,
        file_path: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let ours_lines = split_lines(&ours_text);
        let theirs_lines = split_lines(&theirs_text);
        let base_lines = base_text.as_ref().map(|t| split_lines(t));

        Self {
            conflict,
            editor,
            file_path,
            ours_lines,
            theirs_lines,
            base_lines,
            focus_handle: cx.focus_handle(),
            ours_scroll_handle: ScrollHandle::new(),
            theirs_scroll_handle: ScrollHandle::new(),
            base_scroll_handle: ScrollHandle::new(),
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn accept_ours(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let conflict = self.conflict.clone();
        let ours = conflict.ours.clone();
        crate::conflict_view::resolve_conflict(editor, conflict, vec![ours], window, cx).detach();
        cx.emit(DismissEvent);
    }

    fn accept_theirs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let conflict = self.conflict.clone();
        let theirs = conflict.theirs.clone();
        crate::conflict_view::resolve_conflict(editor, conflict, vec![theirs], window, cx).detach();
        cx.emit(DismissEvent);
    }

    fn accept_both(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let conflict = self.conflict.clone();
        let ours = conflict.ours.clone();
        let theirs = conflict.theirs.clone();
        crate::conflict_view::resolve_conflict(editor, conflict, vec![ours, theirs], window, cx)
            .detach();
        cx.emit(DismissEvent);
    }

    fn accept_base(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(base) = self.conflict.base.clone() {
            let editor = self.editor.clone();
            let conflict = self.conflict.clone();
            crate::conflict_view::resolve_conflict(editor, conflict, vec![base], window, cx)
                .detach();
            cx.emit(DismissEvent);
        }
    }
}

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec!["(empty)".to_string()];
    }
    text.lines().map(|l| l.to_string()).collect()
}

impl EventEmitter<DismissEvent> for ThreeWayConflictModal {}
impl ModalView for ThreeWayConflictModal {}

impl Focusable for ThreeWayConflictModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ThreeWayConflictModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_base = self.base_lines.is_some();
        let file_name = self
            .file_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name()?.to_str())
            .unwrap_or("Conflicted File");

        v_flex()
            .key_context({
                let mut key_context = KeyContext::new_with_defaults();
                key_context.add("ThreeWayConflictModal");
                key_context
            })
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .w(if has_base { rems(62.) } else { rems(54.) })
            .max_h(rems(38.))
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
                                Icon::new(IconName::GitMergeConflict)
                                    .color(Color::Warning)
                                    .size(IconSize::Medium),
                            )
                            .child(
                                v_flex()
                                    .child(
                                        Headline::new(translate_ui(
                                            "3-Way Conflict Comparison",
                                            cx,
                                        ))
                                        .size(HeadlineSize::Small),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} ({} ⟷ {})",
                                            file_name,
                                            self.conflict.ours_branch_name,
                                            self.conflict.theirs_branch_name
                                        ))
                                        .size(LabelSize::XSmall)
                                        .color(Color::Muted),
                                    ),
                            ),
                    )
                    .child(
                        IconButton::new("close-three-way-modal", IconName::Close)
                            .icon_size(IconSize::Small)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .flex_1()
                    .min_h(rems(18.))
                    .max_h(rems(24.))
                    // Column 1: Ours (Current Change)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().status().info_border)
                            .bg(cx.theme().colors().element_background)
                            .child(
                                h_flex()
                                    .px_2p5()
                                    .py_1p5()
                                    .justify_between()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(cx.theme().status().info_border)
                                    .bg(cx.theme().status().info_background.opacity(0.12))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::ArrowLeft)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Accent),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "{}: {}",
                                                    translate_ui("Current Change", cx),
                                                    self.conflict.ours_branch_name
                                                ))
                                                .size(LabelSize::XSmall)
                                                .weight(FontWeight::SEMIBOLD)
                                                .color(Color::Accent),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            "accept-ours-head",
                                            translate_ui("Accept Current", cx),
                                        )
                                        .size(ButtonSize::Compact)
                                        .style(ButtonStyle::OutlinedGhost)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.accept_ours(window, cx);
                                        })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .id("three-way-ours-content")
                                    .font_buffer(cx)
                                    .flex_1()
                                    .p_2()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.ours_scroll_handle)
                                    .children(self.ours_lines.iter().enumerate().map(
                                        |(ix, line)| {
                                            h_flex()
                                                .gap_2()
                                                .items_start()
                                                .child(
                                                    Label::new(format!("{:3}", ix + 1))
                                                        .size(LabelSize::XSmall)
                                                        .color(Color::Muted),
                                                )
                                                .child(
                                                    Label::new(line.clone())
                                                        .size(LabelSize::XSmall),
                                                )
                                        },
                                    )),
                            ),
                    )
                    // Column 2 (Optional): Base (Common Ancestor)
                    .when_some(self.base_lines.clone(), |this, base_lines| {
                        this.child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().colors().border_variant)
                                .bg(cx.theme().colors().element_background)
                                .child(
                                    h_flex()
                                        .px_2p5()
                                        .py_1p5()
                                        .justify_between()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(cx.theme().colors().border_variant)
                                        .child(
                                            Label::new(translate_ui("Base (Ancestor)", cx))
                                                .size(LabelSize::XSmall)
                                                .weight(FontWeight::SEMIBOLD)
                                                .color(Color::Muted),
                                        )
                                        .child(
                                            Button::new(
                                                "accept-base-head",
                                                translate_ui("Accept Base", cx),
                                            )
                                            .size(ButtonSize::Compact)
                                            .style(ButtonStyle::Subtle)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.accept_base(window, cx);
                                            })),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .id("three-way-base-content")
                                        .font_buffer(cx)
                                        .flex_1()
                                        .p_2()
                                        .overflow_y_scroll()
                                        .track_scroll(&self.base_scroll_handle)
                                        .children(base_lines.iter().enumerate().map(
                                            |(ix, line)| {
                                                h_flex()
                                                    .gap_2()
                                                    .items_start()
                                                    .child(
                                                        Label::new(format!("{:3}", ix + 1))
                                                            .size(LabelSize::XSmall)
                                                            .color(Color::Muted),
                                                    )
                                                    .child(
                                                        Label::new(line.clone())
                                                            .size(LabelSize::XSmall),
                                                    )
                                            },
                                        )),
                                ),
                        )
                    })
                    // Column 3: Theirs (Incoming Change)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().status().warning_border)
                            .bg(cx.theme().colors().element_background)
                            .child(
                                h_flex()
                                    .px_2p5()
                                    .py_1p5()
                                    .justify_between()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(cx.theme().status().warning_border)
                                    .bg(cx.theme().status().warning_background.opacity(0.12))
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Icon::new(IconName::ArrowRight)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Warning),
                                            )
                                            .child(
                                                Label::new(format!(
                                                    "{}: {}",
                                                    translate_ui("Incoming Change", cx),
                                                    self.conflict.theirs_branch_name
                                                ))
                                                .size(LabelSize::XSmall)
                                                .weight(FontWeight::SEMIBOLD)
                                                .color(Color::Warning),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            "accept-theirs-head",
                                            translate_ui("Accept Incoming", cx),
                                        )
                                        .size(ButtonSize::Compact)
                                        .style(ButtonStyle::OutlinedGhost)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.accept_theirs(window, cx);
                                        })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .id("three-way-theirs-content")
                                    .font_buffer(cx)
                                    .flex_1()
                                    .p_2()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.theirs_scroll_handle)
                                    .children(self.theirs_lines.iter().enumerate().map(
                                        |(ix, line)| {
                                            h_flex()
                                                .gap_2()
                                                .items_start()
                                                .child(
                                                    Label::new(format!("{:3}", ix + 1))
                                                        .size(LabelSize::XSmall)
                                                        .color(Color::Muted),
                                                )
                                                .child(
                                                    Label::new(line.clone())
                                                        .size(LabelSize::XSmall),
                                                )
                                        },
                                    )),
                            ),
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
                        Label::new(format!(
                            "{}: {} | {}: {}",
                            translate_ui("Current Change", cx),
                            self.ours_lines.len(),
                            translate_ui("Incoming Change", cx),
                            self.theirs_lines.len()
                        ))
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new(
                                    "bottom-accept-both",
                                    translate_ui("Accept Both (Current + Incoming)", cx),
                                )
                                .size(ButtonSize::Compact)
                                .style(ButtonStyle::OutlinedGhost)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.accept_both(window, cx);
                                })),
                            )
                            .child(
                                Button::new(
                                    "bottom-accept-ours",
                                    translate_ui("Accept Current", cx),
                                )
                                .size(ButtonSize::Compact)
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.accept_ours(window, cx);
                                })),
                            )
                            .child(
                                Button::new(
                                    "bottom-accept-theirs",
                                    translate_ui("Accept Incoming", cx),
                                )
                                .size(ButtonSize::Compact)
                                .style(ButtonStyle::Filled)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.accept_theirs(window, cx);
                                })),
                            )
                            .child(
                                Button::new("bottom-close", translate_ui("Close", cx))
                                    .size(ButtonSize::Compact)
                                    .style(ButtonStyle::Subtle)
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_lines_empty() {
        assert_eq!(split_lines(""), vec!["(empty)".to_string()]);
    }

    #[test]
    fn test_split_lines_multiline() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let lines = split_lines(content);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "fn main() {");
        assert_eq!(lines[1], "    println!(\"hello\");");
        assert_eq!(lines[2], "}");
    }
}

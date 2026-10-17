use editor::{Editor, EditorElement, EditorStyle};
use gpui::{App, Entity, IntoElement, TextStyle, Window, div, relative, rems};
use menu::Cancel;
use settings::Settings;
use theme_settings::ThemeSettings;
use ui::{Color, IconName, Tooltip, prelude::*};

/// Build a single-line filter editor with the given placeholder.
pub(crate) fn new_filter_editor_with_placeholder(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Editor> {
    cx.new(|cx| {
        let mut editor = Editor::single_line(window, cx);
        editor.set_placeholder_text(placeholder, window, cx);
        editor
    })
}

/// Current text of a filter editor.
pub(crate) fn filter_query(editor: &Entity<Editor>, cx: &App) -> String {
    editor.read(cx).text(cx)
}

/// Render the shared single-line filter editor (magnifying-glass icon + editor).
pub(crate) fn render_filter_editor(filter_editor: &Entity<Editor>, cx: &App) -> impl IntoElement {
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
        .on_action({
            let filter_editor = filter_editor.clone();
            move |_: &Cancel, window, cx| {
                if !filter_editor.read(cx).text(cx).is_empty() {
                    filter_editor.update(cx, |editor, cx| {
                        editor.set_text("", window, cx);
                    });
                }
            }
        })
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
        .when(!filter_editor.read(cx).text(cx).is_empty(), |this| {
            let filter_editor = filter_editor.clone();
            this.child(
                IconButton::new("gm-clear-filter", IconName::Close)
                    .icon_size(IconSize::XSmall)
                    .tooltip(Tooltip::text(settings::translate_ui("Clear Filter", cx)))
                    .on_click(move |_, window, cx| {
                        filter_editor.update(cx, |editor, cx| {
                            editor.set_text("", window, cx);
                        });
                    }),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use settings::SettingsStore;

    fn init_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            editor::init(cx);
        });
    }

    #[gpui::test]
    fn test_filter_query(cx: &mut TestAppContext) {
        init_test(cx);
        let (editor, cx) = cx.add_window_view(|window, cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text("test-query", window, cx);
            editor
        });
        cx.read(|cx| {
            assert_eq!(filter_query(&editor, cx), "test-query");
        });
    }
}

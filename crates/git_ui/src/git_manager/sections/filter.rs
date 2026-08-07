use editor::{Editor, EditorElement, EditorStyle};
use gpui::{App, Entity, IntoElement, TextStyle, Window, div, relative, rems};
use settings::Settings;
use theme_settings::ThemeSettings;
use ui::{Color, IconName, prelude::*};

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

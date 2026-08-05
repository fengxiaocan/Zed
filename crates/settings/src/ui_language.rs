use settings_content::{SettingsContent, UiLanguage};

use crate::{self as settings, RegisterSetting, Settings};

/// The currently selected language for Zed's user interface.
#[derive(Copy, Clone, Debug, PartialEq, Eq, RegisterSetting)]
pub struct UiLanguageSetting(pub UiLanguage);

impl Settings for UiLanguageSetting {
    fn from_settings(content: &SettingsContent) -> Self {
        Self(content.ui_language.unwrap_or_default())
    }
}

/// Translates a stable English UI string into the currently selected interface
/// language, falling back to English when no translation exists. The English
/// string remains the canonical identifier; translation is applied only while
/// rendering.
///
/// This is a convenience wrapper around [`UiLanguage::translate`] that reads
/// the active language from the global settings store, so feature crates (git
/// panel, etc.) don't each have to thread the language through their views.
pub fn translate_ui(text: &'static str, cx: &gpui::App) -> &'static str {
    use crate::Settings as _;
    UiLanguageSetting::get_global(cx).0.translate(text)
}

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

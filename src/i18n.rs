//! UI localization: `rust-i18n` dictionaries from `locales/*.json`
//! plus system-language detection (`sys-locale`).
//!
//! Detection runs ONLY when the config has no stored `language`
//! (first launch or pre-i18n configs). Afterwards the stored value
//! wins and can only be changed in Settings.
//!
//! NOTE: the `rust_i18n::i18n!` inventory macro itself lives in the crate
//! root (`main.rs`) — that is required for `t!` to resolve in every module.

/// `t!` for widget args that borrow (`text_input` placeholder takes
/// `&str`). Only for keys WITHOUT interpolation: those always resolve
/// to compile-time `&'static str`, so no allocation and no lifetime issue.
#[macro_export]
macro_rules! tr {
    ($key:literal) => {
        match rust_i18n::t!($key) {
            std::borrow::Cow::Borrowed(s) => s,
            std::borrow::Cow::Owned(_) => concat!("<!", $key, ">"),
        }
    };
}

/// Supported language codes.
pub const ENGLISH: &str = "en";
pub const RUSSIAN: &str = "ru";

/// System language: Russian when the OS locale starts with "ru",
/// English for anything else.
pub fn detect_system_language() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    if locale.starts_with("ru") {
        RUSSIAN.to_owned()
    } else {
        ENGLISH.to_owned()
    }
}

/// Stored config value -> valid code. Unknown/empty -> English.
pub fn sanitize_language(code: &str) -> String {
    match code {
        RUSSIAN => RUSSIAN.to_owned(),
        ENGLISH => ENGLISH.to_owned(),
        _ => ENGLISH.to_owned(),
    }
}

/// Apply the language globally (all subsequent `t!` calls use it).
pub fn apply(code: &str) {
    rust_i18n::set_locale(code);
}

/// Display name for the Settings dropdown.
pub fn display_name(code: &str) -> String {
    match code {
        RUSSIAN => String::from("Русский"),
        _ => String::from("English"),
    }
}

/// Dropdown display name -> language code.
pub fn code_for_display(label: &str) -> String {
    match label {
        "Русский" => RUSSIAN.to_owned(),
        _ => ENGLISH.to_owned(),
    }
}

#[cfg(test)]
mod i18n_tests {
    use super::*;

    #[test]
    fn sanitize_rejects_unknown() {
        assert_eq!(sanitize_language("ru"), "ru");
        assert_eq!(sanitize_language("en"), "en");
        assert_eq!(sanitize_language(""), "en");
        assert_eq!(sanitize_language("de"), "en");
    }

    #[test]
    fn display_roundtrip() {
        assert_eq!(code_for_display(&display_name("ru")), "ru");
        assert_eq!(code_for_display(&display_name("en")), "en");
        assert_eq!(code_for_display("???"), "en");
    }

    #[test]
    fn dictionaries_have_settings_title() {
        // Explicit locale param: no global-state mutation, race-free.
        assert_eq!(
            rust_i18n::t!("settings.title", locale = "en").to_string(),
            "Settings"
        );
        assert_eq!(
            rust_i18n::t!("settings.title", locale = "ru").to_string(),
            "Настройки"
        );
    }

    #[test]
    fn interpolation_works_in_both_locales() {
        assert_eq!(
            rust_i18n::t!("install.progress", locale = "en", done = 1, total = 2, pct = 50)
                .to_string(),
            "Downloaded 1 from 2 files. (50%)"
        );
        assert_eq!(
            rust_i18n::t!("install.progress", locale = "ru", done = 1, total = 2, pct = 50)
                .to_string(),
            "Загружено 1 из 2 файлов. (50%)"
        );
    }
}

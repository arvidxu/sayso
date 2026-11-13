// Menu bar texts are the "native" part of the locale files the page translates from.
use std::sync::{OnceLock, RwLock};

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

// Shared with the page; tests keep `LANGUAGES` and Info.plist in step with it.
const LIST: &str = include_str!("../../src/i18n/languages.json");

fn list() -> &'static Value {
    static PARSED: OnceLock<Value> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(LIST).expect("languages.json is not valid JSON"))
}

// The system's tag for an app language, such as "zh-Hans" for "zh-CN".
#[cfg(target_os = "macos")]
pub fn macos_tag(language: &str) -> Option<&'static str> {
    list().get(language)?.get("macos")?.as_str()
}

// Same order as languages.json. The first is the fallback for a missing language or text.
const LANGUAGES: [(&str, &str); 3] = [
    ("en", include_str!("../../src/i18n/locales/en.json")),
    ("zh-CN", include_str!("../../src/i18n/locales/zh-CN.json")),
    ("zh-TW", include_str!("../../src/i18n/locales/zh-TW.json")),
];

static CURRENT: RwLock<usize> = RwLock::new(0);

fn texts() -> &'static [Value] {
    static TEXTS: OnceLock<Vec<Value>> = OnceLock::new();
    TEXTS.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|(_, json)| serde_json::from_str(json).expect("a locale file is not valid JSON"))
            .collect()
    })
}

fn index_of(language: &str) -> Option<usize> {
    LANGUAGES.iter().position(|(code, _)| *code == language)
}

// Returns whether the language changed.
fn select(index: usize) -> bool {
    let mut current = CURRENT
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let is_changed = *current != index;
    *current = index;
    is_changed
}

// Runs before the page loads so the menu bar starts in the right language.
pub fn setup(app: &AppHandle) {
    let chosen = app
        .store(crate::SETTINGS)
        .ok()
        .and_then(|settings| settings.get("language"))
        .and_then(|value| value.as_str().and_then(index_of));
    select(chosen.or_else(|| index_of(system())).unwrap_or(0));
}

// Returns whether the language changed; an unsupported one changes nothing.
pub fn set(language: &str) -> bool {
    index_of(language).is_some_and(select)
}

pub fn system() -> &'static str {
    #[cfg(target_os = "macos")]
    let tags = crate::system_language::languages();
    #[cfg(not(target_os = "macos"))]
    let tags: Vec<String> = sys_locale::get_locales().collect();
    tags.iter()
        .find_map(|tag| from_system(tag))
        .unwrap_or(LANGUAGES[0].0)
}

// Matches by language and script; a tag without a script matches by the "regions" in
// languages.json, or else by language alone.
fn from_system(tag: &str) -> Option<&'static str> {
    let tag = tag.to_lowercase();
    let mut subtags = tag.split(['-', '_']);
    let language = subtags.next()?;
    let rest: Vec<&str> = subtags.collect();
    // A script subtag has four letters; a region two letters or three digits.
    fn script_of<'a>(subtags: &[&'a str]) -> Option<&'a str> {
        subtags.iter().copied().find(|subtag| subtag.len() == 4)
    }
    let script = script_of(&rest).map(str::to_owned);
    let candidates: Vec<(&'static str, Option<String>)> = LANGUAGES
        .iter()
        .filter_map(|(code, _)| {
            let macos = list()[*code]["macos"].as_str()?.to_lowercase();
            let parts: Vec<&str> = macos.split('-').collect();
            (parts[0] == language).then(|| (*code, script_of(&parts[1..]).map(str::to_owned)))
        })
        .collect();
    let in_region = |code: &str| {
        list()[code]["regions"].as_array().is_some_and(|regions| {
            regions
                .iter()
                .filter_map(Value::as_str)
                .any(|region| rest.contains(&region.to_lowercase().as_str()))
        })
    };
    let found = match script {
        Some(script) => candidates
            .iter()
            .find(|(_, own)| own.as_ref().is_none_or(|own| *own == script)),
        None => candidates
            .iter()
            .find(|(code, _)| in_region(code))
            .or(candidates.first()),
    };
    found.map(|(code, _)| *code)
}

// Texts the system words differently, as "Copy" in Chinese; holds only those that differ.
const ON_MACOS: &str = "nativeOnMacos";

fn lookup_in(index: usize, part: &str, key: &str) -> Option<&'static str> {
    key.split('.')
        .try_fold(texts()[index].get(part)?, |value, part| value.get(part))?
        .as_str()
}

fn lookup(index: usize, key: &str) -> Option<&'static str> {
    let on_macos = cfg!(target_os = "macos")
        .then(|| lookup_in(index, ON_MACOS, key))
        .flatten();
    on_macos.or_else(|| lookup_in(index, "native", key))
}

// Replaces each "{{name}}" with its value.
pub fn text(key: &str, values: &[(&str, &str)]) -> String {
    let current = *CURRENT
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let template = lookup(current, key)
        .or_else(|| lookup(0, key))
        .unwrap_or(key);
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{{{name}}}}}"), value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_native_text() {
        fn keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
            match value.as_object() {
                Some(map) => map
                    .iter()
                    .for_each(|(key, value)| keys(value, &format!("{prefix}.{key}"), out)),
                None => out.push(prefix.to_owned()),
            }
        }
        let mut first = Vec::new();
        keys(&texts()[0]["native"], "", &mut first);
        assert!(!first.is_empty());
        for (index, (code, _)) in LANGUAGES.iter().enumerate() {
            let mut other = Vec::new();
            keys(&texts()[index]["native"], "", &mut other);
            assert_eq!(first, other, "{code} differs from {}", LANGUAGES[0].0);
        }
    }

    #[test]
    fn macos_wordings_replace_existing_texts() {
        for (index, (code, _)) in LANGUAGES.iter().enumerate() {
            let Some(menu) = texts()[index]
                .get(ON_MACOS)
                .and_then(|part| part.get("menu"))
            else {
                continue;
            };
            for key in menu.as_object().unwrap().keys() {
                let key = format!("menu.{key}");
                assert!(lookup_in(index, "native", &key).is_some(), "{code} {key}");
            }
        }
    }

    #[test]
    fn copy_is_worded_for_the_system() {
        let simplified = LANGUAGES
            .iter()
            .position(|(code, _)| *code == "zh-CN")
            .unwrap();
        let expected = if cfg!(target_os = "macos") {
            "拷贝"
        } else {
            "复制"
        };
        assert_eq!(lookup(simplified, "menu.copy"), Some(expected));
    }

    #[test]
    fn the_translations_are_those_of_the_language_list() {
        let mut listed: Vec<&str> = list()
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        // In the order of the file, which the parsed list does not keep.
        listed.sort_by_key(|code| LIST.find(&format!("\"{code}\": {{")));
        let loaded: Vec<&str> = LANGUAGES.iter().map(|(code, _)| *code).collect();
        assert_eq!(listed, loaded);
    }

    // The system shows the menu items it adds in one of the languages Info.plist declares.
    #[test]
    fn info_plist_declares_every_language() {
        let plist = include_str!("../Info.plist");
        let localizations = plist
            .split_once("<key>CFBundleLocalizations</key>")
            .and_then(|(_, rest)| rest.split_once("</array>"))
            .map(|(array, _)| array)
            .expect("Info.plist has no CFBundleLocalizations");
        let mut declared: Vec<&str> = localizations
            .split("<string>")
            .skip(1)
            .filter_map(|part| part.split_once("</string>"))
            .map(|(tag, _)| tag)
            .collect();
        let mut wanted: Vec<&str> = list()
            .as_object()
            .unwrap()
            .values()
            .map(|language| language["macos"].as_str().unwrap())
            .collect();
        declared.sort_unstable();
        wanted.sort_unstable();
        assert_eq!(declared, wanted);
    }

    #[test]
    fn system_languages_map_to_the_app_languages() {
        for (tag, language) in [
            ("en-AU", Some("en")),
            ("zh-Hans-CN", Some("zh-CN")),
            ("zh-Hant-HK", Some("zh-TW")),
            ("zh-Hans-HK", Some("zh-CN")),
            ("zh_TW", Some("zh-TW")),
            ("zh-HK", Some("zh-TW")),
            ("zh-SG", Some("zh-CN")),
            ("zh", Some("zh-CN")),
            ("fr-FR", None),
        ] {
            assert_eq!(from_system(tag), language, "{tag}");
        }
    }

    #[test]
    fn text_fills_in_values() {
        assert_eq!(text("menu.quit", &[("name", "Sayso")]), "Quit Sayso");
        assert_eq!(text("no.such.key", &[]), "no.such.key");
    }
}

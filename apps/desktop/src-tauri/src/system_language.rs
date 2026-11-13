// The system picks the language of its own menu items and panels once, at launch, from the app's
// preferred languages, so a language picked in the settings takes effect on the next start.
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{
    NSArray, NSDateFormatter, NSGlobalDomain, NSLocale, NSString, NSUserDefaults, ns_string,
};

fn key() -> &'static NSString {
    ns_string!("AppleLanguages")
}

// The page cannot tell: once the app has its own preferred language, the page is given that one.
pub fn languages() -> Vec<String> {
    let defaults = NSUserDefaults::standardUserDefaults();
    let Some(global) = defaults.persistentDomainForName(unsafe { NSGlobalDomain }) else {
        return Vec::new();
    };
    let Some(value) = global.objectForKey(key()) else {
        return Vec::new();
    };
    let Ok(list) = value.downcast::<NSArray>() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item: Retained<AnyObject>| item.downcast::<NSString>().ok())
        .map(|language| language.to_string())
        .collect()
}

// The page only knows the language's usual clock, not the user's choice. The "j" template gives
// the preferred hour: "h" or "K" on a 12-hour clock; quoted text is not part of the pattern.
pub fn uses_12_hour_clock() -> bool {
    NSDateFormatter::dateFormatFromTemplate_options_locale(
        ns_string!("j"),
        0,
        Some(&NSLocale::currentLocale()),
    )
    .is_some_and(|format| {
        format
            .to_string()
            .split('\'')
            .step_by(2)
            .any(|part| part.contains(['h', 'K']))
    })
}

// `language` is one of the app's own codes; `None` follows the system again.
pub fn set_preferred(language: Option<&str>) {
    let defaults = NSUserDefaults::standardUserDefaults();
    let Some(language) = language else {
        defaults.removeObjectForKey(key());
        return;
    };
    let tag = crate::i18n::macos_tag(language).unwrap_or(language);
    let list = NSArray::from_retained_slice(&[NSString::from_str(tag)]);
    // SAFETY: an array of strings is a property list object, which is what a default holds.
    unsafe { defaults.setObject_forKey(Some(&list), key()) };
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_system_languages() {
        let languages = super::languages();
        println!("system languages: {languages:?}");
        assert!(!languages.is_empty());
    }
}

/// (replaced, current) codes for the same language: withdrawn codes, and `tl`, written as
/// `fil`.
const CODE_ALIASES: &[(&str, &str)] = &[
    ("iw", "he"),
    ("in", "id"),
    ("ji", "yi"),
    ("jw", "jv"),
    ("mo", "ro"),
    ("tl", "fil"),
];

/// (macrolanguage, individual language) where the macrolanguage code is written for that
/// language. Other members, such as `yue`, `arz` or `nn`, are separate languages.
const MACROLANGUAGES: &[(&str, &str)] = &[
    ("ak", "tw"),
    ("ar", "arb"),
    ("ay", "ayr"),
    ("az", "azj"),
    ("bal", "bcc"),
    ("bik", "bcl"),
    ("bua", "bxr"),
    ("chm", "mhr"),
    ("din", "dik"),
    ("doi", "dgo"),
    ("et", "ekk"),
    ("fa", "pes"),
    ("ff", "fuc"),
    ("gn", "gug"),
    ("ik", "esk"),
    ("iu", "ike"),
    ("kg", "kng"),
    ("kok", "knn"),
    ("kr", "knc"),
    ("ku", "kmr"),
    ("kv", "kpv"),
    ("lah", "pnb"),
    ("lv", "lvs"),
    ("man", "emk"),
    ("mg", "plt"),
    ("mn", "khk"),
    ("ms", "zsm"),
    ("ne", "npi"),
    ("no", "nb"),
    ("oj", "ojg"),
    ("om", "gaz"),
    ("or", "ory"),
    ("ps", "pbu"),
    ("qu", "quz"),
    ("rom", "rmy"),
    ("sc", "src"),
    ("sq", "als"),
    ("sw", "swh"),
    ("uz", "uzn"),
    ("yi", "ydd"),
    ("za", "zyb"),
    ("zh", "cmn"),
    ("zza", "diq"),
];

/// Replaced code to current, then to the macrolanguage code standing for it; others unchanged.
pub(crate) fn canonical_code(code: &str) -> &str {
    let current = CODE_ALIASES
        .iter()
        .find(|(replaced, _)| *replaced == code)
        .map_or(code, |(_, current)| current);
    MACROLANGUAGES
        .iter()
        .find(|(_, individual)| *individual == current)
        .map_or(current, |(macrolanguage, _)| macrolanguage)
}

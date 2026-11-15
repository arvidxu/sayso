use std::collections::HashSet;

use dictation_types::{ErrorKind, ModelId};

macro_rules! identifier_requirements {
    ($($module:ident => $ty:ident),+ $(,)?) => {$(
        mod $module {
            use super::*;

            #[test]
            fn surrounding_whitespace_never_makes_two_ids_different() {
                let plain = $ty::new("some-id");
                for padded in [" some-id", "some-id ", "  some-id  ", "\tsome-id\n", "\u{3000}some-id\u{00A0}"] {
                    assert_eq!($ty::new(padded), plain, "{padded:?}");
                    assert_eq!($ty::from(padded), plain, "{padded:?}");
                    assert_eq!($ty::from(padded.to_owned()), plain, "{padded:?}");
                    assert_eq!($ty::try_new(padded).unwrap(), plain, "{padded:?}");
                    assert_eq!($ty::new(padded).as_str(), "some-id");
                }
            }

            #[test]
            fn padded_and_plain_ids_are_one_key() {
                let set: HashSet<$ty> = ["id", " id", "id ", " id "].into_iter().map($ty::new).collect();
                assert_eq!(set.len(), 1);
            }

            #[test]
            fn validated_construction_rejects_blank_input() {
                for blank in ["", " ", "   ", "\t", "\n", "\u{3000}"] {
                    let err = $ty::try_new(blank).unwrap_err();
                    assert_eq!(err.kind, ErrorKind::Validation, "{blank:?}");
                }
            }

            #[test]
            fn the_id_text_is_otherwise_kept_exactly() {
                for text in ["Mixed-Case_ID", "with inner space", "идентификатор", "식별자", "標識/子", "a"] {
                    let id = $ty::try_new(text).unwrap();
                    assert_eq!(id.as_str(), text);
                    assert_eq!(id.to_string(), text);
                    let borrowed: &str = id.as_ref();
                    assert_eq!(borrowed, text);
                    assert!(!id.is_empty());
                    assert_eq!(id.into_inner(), text);
                }
            }

            #[test]
            fn ids_differing_in_letter_case_are_different_ids() {
                assert_ne!($ty::new("abc"), $ty::new("ABC"));
            }
        }
    )+};
}

// Every ID type comes from one macro, so one of them stands for all.
identifier_requirements!(model_id => ModelId);

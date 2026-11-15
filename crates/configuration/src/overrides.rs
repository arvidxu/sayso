use std::fmt;

use dictation_types::{AppError, AppResult};
use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value, from_str, to_string};

/// JSON read strictly: a key that appears twice in an object is an error instead of the later
/// one silently winning.
pub(crate) struct Strict(pub(crate) Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(Strict)
    }
}

struct StrictSeed;

impl<'de> DeserializeSeed<'de> for StrictSeed {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: DeError>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("a number that is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(StrictSeed)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(A::Error::custom(format!("the key '{key}' appears twice")));
            }
            let value = map.next_value_seed(StrictSeed)?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

/// The package configuration `base` with each of `layers` merged over it in order: objects
/// merge, anything else (arrays included) replaces. An unknown key or a change of kind fails.
pub(crate) fn apply(package: &str, base: &str, layers: &[&Value]) -> AppResult<String> {
    // The package's own file is read strictly even without overrides, so that a broken file
    // fails the same way whether or not the user has overrides.
    let mut merged = from_str::<Strict>(base)
        .map_err(|err| {
            AppError::internal(format!(
                "the configuration the {package} package ships with is not valid JSON: {err}"
            ))
        })?
        .0;
    for layer in layers {
        merge(
            &mut merged,
            layer,
            &format!("dictation.overrides.{package}"),
        )?;
    }
    to_string(&merged)
        .map_err(|err| AppError::internal(format!("failed to merge overrides: {err}")))
}

/// What kind of JSON value `value` is, for error messages.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "a number",
        Value::String(_) => "text",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

fn merge(target: &mut Value, overrides: &Value, at: &str) -> AppResult<()> {
    let (Value::Object(target), Value::Object(overrides)) = (target, overrides) else {
        return Err(AppError::validation(format!("{at} must be an object")));
    };
    for (key, value) in overrides {
        let here = format!("{at}.{key}");
        let Some(existing) = target.get_mut(key) else {
            return Err(AppError::validation(format!(
                "{here} overrides a value the package does not have"
            )));
        };
        if existing.is_object() && value.is_object() {
            merge(existing, value, &here)?;
            continue;
        }
        // A value that may be null in the package's file can be given any kind; every other
        // value keeps its kind.
        if !existing.is_null() && kind(existing) != kind(value) {
            return Err(AppError::validation(format!(
                "{here} must be {}, not {}",
                kind(existing),
                kind(value)
            )));
        }
        *existing = value.clone();
    }
    Ok(())
}

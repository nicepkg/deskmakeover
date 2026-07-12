//! The validated appearance recipe shared by the two persistence stores that hold one.
//!
//! `IconStyle` is the three global style knobs the frontend authors — `{config, kindPolicy,
//! typeOverrides}`, matching `src/bridge/types.ts` `ConfigDto`/`KindPolicy`/`TypeOverrides`. It is
//! persisted verbatim as store ② (saved-style, `SettingsStore`) and inside each store ③ entry
//! (look-history), and compared field-for-field for the history dedup rule (spec 07 §17).
//!
//! The FIELD INTERNALS stay frontend-owned and opaque to Rust — the same ownership line D1 drew for
//! wallpaper; only the native bake path reads inside `config`. But the ENVELOPE is validated on the
//! way in so a malformed value can never be persisted AS "a saved style" (which M7 would then read
//! as ② non-empty with nothing to project): it must be a JSON object carrying object-typed
//! `config`, `kindPolicy`, and `typeOverrides`, and must NOT carry per-icon `overrides` (spec 07
//! §8.2 — a per-icon override cannot apply to an icon that did not exist when the style was saved).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The three top-level keys every appearance recipe must carry, each an object.
const REQUIRED_OBJECT_FIELDS: [&str; 3] = ["config", "kindPolicy", "typeOverrides"];

/// A validated appearance recipe. Construct via [`IconStyle::from_value`]; the inner blob is only
/// ever a value that passed [`validate`]. Deserialization (from the settings blob / look-history)
/// runs the same validation, so a persisted recipe is always well-formed or rejected — never
/// silently accepted as garbage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub struct IconStyle(Value);

impl IconStyle {
    /// Validates `value` as an appearance recipe (see the module docs) and wraps it.
    pub fn from_value(value: Value) -> Result<Self, IconStyleError> {
        validate(&value)?;
        Ok(Self(value))
    }

    /// The underlying `{config, kindPolicy, typeOverrides}` blob.
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// Consumes the recipe, yielding its blob.
    pub fn into_value(self) -> Value {
        self.0
    }
}

/// Why a value is not a valid [`IconStyle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconStyleError {
    /// The value was not a JSON object (e.g. `null`, a string, a number).
    NotAnObject,
    /// A required top-level field was absent.
    MissingField(&'static str),
    /// A required top-level field was present but not an object.
    FieldNotObject(&'static str),
    /// The recipe carried per-icon `overrides`, which store ② must never hold (spec 07 §8.2).
    PerIconOverrides,
}

impl std::fmt::Display for IconStyleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "icon style is not a JSON object"),
            Self::MissingField(k) => write!(f, "icon style is missing the {k:?} field"),
            Self::FieldNotObject(k) => write!(f, "icon style {k:?} field is not an object"),
            Self::PerIconOverrides => {
                write!(f, "icon style must not carry per-icon overrides (spec 07 §8.2)")
            }
        }
    }
}

impl std::error::Error for IconStyleError {}

impl TryFrom<Value> for IconStyle {
    type Error = IconStyleError;
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::from_value(value)
    }
}

impl From<IconStyle> for Value {
    fn from(style: IconStyle) -> Self {
        style.0
    }
}

/// Validates the recipe envelope: an object with object-typed `config`/`kindPolicy`/`typeOverrides`
/// and no per-icon `overrides`. Deliberately does NOT validate the field internals (frontend-owned)
/// and tolerates additional forward-compat keys, matching the DTO decode discipline elsewhere.
fn validate(value: &Value) -> Result<(), IconStyleError> {
    let obj = value.as_object().ok_or(IconStyleError::NotAnObject)?;
    for key in REQUIRED_OBJECT_FIELDS {
        match obj.get(key) {
            Some(Value::Object(_)) => {}
            Some(_) => return Err(IconStyleError::FieldNotObject(key)),
            None => return Err(IconStyleError::MissingField(key)),
        }
    }
    if obj.contains_key("overrides") {
        return Err(IconStyleError::PerIconOverrides);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid() -> Value {
        json!({ "config": { "shape": "Apple" }, "kindPolicy": {}, "typeOverrides": {} })
    }

    #[test]
    fn accepts_a_well_formed_recipe_and_preserves_it_verbatim() {
        let style = IconStyle::from_value(valid()).unwrap();
        assert_eq!(style.as_value(), &valid());
        // Round-trips through serde as its inner blob (unchanged on-disk shape).
        let json = serde_json::to_string(&style).unwrap();
        let back: IconStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(style, back);
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), valid());
    }

    #[test]
    fn rejects_null_and_non_objects() {
        assert_eq!(IconStyle::from_value(json!(null)).unwrap_err(), IconStyleError::NotAnObject);
        assert_eq!(IconStyle::from_value(json!("nope")).unwrap_err(), IconStyleError::NotAnObject);
        assert_eq!(IconStyle::from_value(json!(42)).unwrap_err(), IconStyleError::NotAnObject);
        // And through serde (the persistence path).
        assert!(serde_json::from_str::<IconStyle>("null").is_err());
    }

    #[test]
    fn rejects_missing_or_non_object_required_fields() {
        assert_eq!(
            IconStyle::from_value(json!({ "kindPolicy": {}, "typeOverrides": {} })).unwrap_err(),
            IconStyleError::MissingField("config"),
        );
        assert_eq!(
            IconStyle::from_value(json!({ "config": "x", "kindPolicy": {}, "typeOverrides": {} }))
                .unwrap_err(),
            IconStyleError::FieldNotObject("config"),
        );
    }

    #[test]
    fn rejects_per_icon_overrides() {
        let with_overrides = json!({
            "config": {}, "kindPolicy": {}, "typeOverrides": {},
            "overrides": { "some-item-id": { "mode": "tint" } }
        });
        assert_eq!(
            IconStyle::from_value(with_overrides).unwrap_err(),
            IconStyleError::PerIconOverrides,
        );
    }

    #[test]
    fn tolerates_forward_compat_extra_keys() {
        let future = json!({ "config": {}, "kindPolicy": {}, "typeOverrides": {}, "future": 9 });
        assert!(IconStyle::from_value(future).is_ok());
    }

    #[test]
    fn equality_is_order_independent_for_dedup() {
        let a = IconStyle::from_value(json!({ "config": { "a": 1, "b": 2 }, "kindPolicy": {}, "typeOverrides": {} })).unwrap();
        let b = IconStyle::from_value(json!({ "config": { "b": 2, "a": 1 }, "kindPolicy": {}, "typeOverrides": {} })).unwrap();
        assert_eq!(a, b, "field-for-field equality must ignore key order (history dedup)");
    }
}

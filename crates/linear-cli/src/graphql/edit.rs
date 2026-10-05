//! Three-state nullable inputs and query variables: omitted, null, or a value.
//!
//! Linear's update inputs distinguish an omitted key (leave the field alone)
//! from an explicit `null` (clear the field, or for `trashed`, restore the
//! issue). A single `Option<T>` cannot express both, so input objects use
//! [`Edit<T>`] for nullable scalar fields together with
//! `#[cynic(skip_serializing_if = "Edit::is_unchanged")]`:
//!
//! | Variant | Wire result |
//! |---|---|
//! | `Unchanged` | key omitted |
//! | `Clear` | `"key": null` |
//! | `Set(value)` | `"key": value` (including `false`, `0`, `""`) |
//!
//! Cynic's `InputObject` derive keeps its compile-time schema check because
//! `Edit<T>` implements the marker traits Cynic asserts on the aligned field
//! type: [`cynic::schema::IsScalar`] and [`cynic::Enum`] forward to `T`, so
//! `Edit<bool>` is accepted for `Boolean` and rejected for `String`. The
//! derive treats `Edit<T>` as a leaf value, so a nullable list field such as
//! `labelIds: [String!]` is not representable as `Edit<Vec<T>>`; list fields
//! use `Option<Vec<T>>` with `skip_serializing_if = "Option::is_none"` (omit
//! or set), which are the only two states this program sends for lists. Nullable input-object fields are not covered yet; add a forwarding
//! `cynic::InputObject` impl together with a derive that uses it.
//!
//! `Edit<T>` is serialization-only. It carries a `Deserialize` impl solely because
//! [`cynic::Enum`] requires `DeserializeOwned`, and that impl always fails:
//! a lenient impl would turn a missing field into `Clear` (serde feeds
//! missing fields through `deserialize_option`), silently converting "leave
//! alone" into "clear" in any future input derive.

use serde::de::{Deserialize, Deserializer, Error as _};
use serde::ser::{Error as _, Serialize, Serializer};

use crate::graphql::schema;

/// A field edit that can leave a value unchanged, clear it, or set it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit<T> {
    /// Leave the field as it is: the key is omitted from the input object.
    Unchanged,
    /// Clear the field: the key is sent with an explicit `null`.
    Clear,
    /// Set the field to this value.
    Set(T),
}

impl<T> Edit<T> {
    /// True when the key must be omitted from the serialized input.
    ///
    /// Use as `#[cynic(skip_serializing_if = "Edit::is_unchanged")]`.
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }

    /// Builds an edit from an optional value: `Some` sets, `None` clears.
    pub fn set_or_clear(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        }
    }

    /// Builds an edit from an optional value: `Some` sets, `None` leaves unchanged.
    pub fn set_or_unchanged(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Set(value),
            None => Self::Unchanged,
        }
    }
}

// A derived `Default` would add a `T: Default` bound, which scalar newtypes
// such as `Json` deliberately do not satisfy; `Unchanged` needs no `T`.
#[allow(clippy::derivable_impls)]
impl<T> Default for Edit<T> {
    fn default() -> Self {
        Self::Unchanged
    }
}

impl<T> From<T> for Edit<T> {
    fn from(value: T) -> Self {
        Self::Set(value)
    }
}

impl<T: Serialize> Serialize for Edit<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // Serializing an unchanged edit would have to invent a wire value,
            // which would silently turn "leave alone" into "clear". Refuse.
            Self::Unchanged => Err(S::Error::custom(
                "Edit::Unchanged must be omitted with skip_serializing_if, not serialized",
            )),
            Self::Clear => serializer.serialize_none(),
            Self::Set(value) => serializer.serialize_some(value),
        }
    }
}

impl<'de, T> Deserialize<'de> for Edit<T> {
    /// Always fails. See the module documentation for why this exists.
    fn deserialize<D: Deserializer<'de>>(_deserializer: D) -> Result<Self, D::Error> {
        Err(D::Error::custom(
            "Edit<T> is a write-only adapter and cannot be deserialized",
        ))
    }
}

impl<T, SchemaType> cynic::schema::IsScalar<SchemaType> for Edit<T>
where
    T: cynic::schema::IsScalar<SchemaType>,
{
    type SchemaType = T::SchemaType;
}

impl<T: cynic::Enum> cynic::Enum for Edit<T> {
    type SchemaType = T::SchemaType;
}

// Query variables need the same nullable schema type and argument coercion as
// Option<T>, while Edit's serializer keeps omission distinct from explicit null.
impl<T: schema::variable::Variable> schema::variable::Variable for Edit<T> {
    const TYPE: cynic::variables::VariableType = cynic::variables::VariableType::Nullable(&T::TYPE);
}

impl<T, L> cynic::coercions::CoercesTo<Option<L>> for Edit<T> where T: cynic::coercions::CoercesTo<L>
{}

#[cfg(test)]
mod tests {
    use super::Edit;
    use serde_json::{Value, json, to_value};

    #[test]
    fn unchanged_refuses_direct_serialization() {
        let error = to_value(Edit::<i32>::Unchanged).expect_err("must not serialize");
        assert!(
            error
                .to_string()
                .contains("Edit::Unchanged must be omitted")
        );
        assert_eq!(to_value(Edit::<i32>::Clear).expect("null"), Value::Null);
        assert_eq!(to_value(Edit::Set(7)).expect("value"), json!(7));
    }

    #[test]
    fn deserialization_always_fails_so_a_missing_field_can_never_become_clear() {
        for body in ["null", "true", "1", r#""x""#] {
            let error = serde_json::from_str::<Edit<bool>>(body).expect_err(body);
            assert!(
                error
                    .to_string()
                    .contains("Edit<T> is a write-only adapter and cannot be deserialized"),
                "{body}: {error}"
            );
        }
        #[derive(serde::Deserialize)]
        struct Probe {
            #[allow(dead_code)]
            trashed: Edit<bool>,
        }
        for body in ["{}", r#"{"trashed":null}"#, r#"{"trashed":true}"#] {
            assert!(serde_json::from_str::<Probe>(body).is_err(), "{body}");
        }
    }

    #[test]
    fn helpers_convert_options_and_default_to_unchanged() {
        assert_eq!(Edit::<i32>::default(), Edit::Unchanged);
        assert_eq!(Edit::set_or_clear(None::<i32>), Edit::Clear);
        assert_eq!(Edit::set_or_clear(Some(1)), Edit::Set(1));
        assert_eq!(Edit::set_or_unchanged(None::<i32>), Edit::Unchanged);
        assert_eq!(Edit::from(2), Edit::Set(2));
        assert!(Edit::<i32>::Unchanged.is_unchanged());
        assert!(!Edit::<i32>::Clear.is_unchanged());
    }
}

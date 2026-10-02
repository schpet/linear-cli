//! Typed views of the schema's `Float` fields.
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Error};

use crate::graphql::schema;

/// A `Float` field the schema uses for a whole quantity: cycle and issue
/// numbers, priorities, counts. Decoding rejects anything that is not a
/// whole number in `u32` range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct WholeNumber(pub u32);

impl<'de> Deserialize<'de> for WholeNumber {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        let whole = match number.as_u64() {
            Some(value) => u32::try_from(value).ok(),
            None => number
                .as_f64()
                .filter(|value| value.fract() == 0.0)
                .and_then(|value| value.to_string().parse::<u32>().ok()),
        };
        whole
            .map(Self)
            .ok_or_else(|| D::Error::custom(format!("expected a whole number, found {number}")))
    }
}

impl fmt::Display for WholeNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

cynic::impl_scalar!(WholeNumber, schema::Float);

/// A fractional `Float` field (sort orders, positions, progress). It keeps the
/// number as received, except that a whole value is held as an integer so it
/// is written back without a fractional part.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Float(pub serde_json::Number);

impl<'de> Deserialize<'de> for Float {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        // `f64` displays whole values without an exponent or fraction, so
        // parsing that text is an exact, range-checked conversion.
        let whole = number
            .as_f64()
            .filter(|value| number.is_f64() && value.fract() == 0.0)
            .and_then(|value| value.to_string().parse::<i64>().ok());
        Ok(Self(whole.map_or(number, serde_json::Number::from)))
    }
}

impl Float {
    pub fn get(&self) -> f64 {
        self.0
            .as_f64()
            .unwrap_or_else(|| unreachable!("JSON numbers are representable as f64"))
    }
}

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

cynic::impl_scalar!(Float, schema::Float);

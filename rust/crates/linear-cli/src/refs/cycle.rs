use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

/// A team's cycle number as written in cycle URLs and cycle arguments:
/// canonical decimal digits (no sign, no leading zero) in `1..=u32::MAX`,
/// the same range the API's whole-number cycle numbers decode into.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CycleNumber(NonZeroU32);

impl CycleNumber {
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for CycleNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CycleNumberProblem {
    /// Empty, or contains something other than ASCII digits.
    NotDigits,
    Zero,
    LeadingZero,
    TooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleNumberError {
    input: String,
    problem: CycleNumberProblem,
}

impl CycleNumberError {
    pub const fn problem(&self) -> CycleNumberProblem {
        self.problem
    }
}

impl fmt::Display for CycleNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let input = &self.input;
        match self.problem {
            CycleNumberProblem::NotDigits => write!(f, "\"{input}\" is not a cycle number"),
            CycleNumberProblem::Zero => {
                write!(
                    f,
                    "\"{input}\" is not a cycle number: cycle numbers start at 1"
                )
            }
            CycleNumberProblem::LeadingZero => write!(
                f,
                "\"{input}\" is not a cycle number: it has a leading zero"
            ),
            CycleNumberProblem::TooLarge => write!(
                f,
                "\"{input}\" is not a cycle number: the largest cycle number is {}",
                u32::MAX
            ),
        }
    }
}

impl std::error::Error for CycleNumberError {}

impl FromStr for CycleNumber {
    type Err = CycleNumberError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let fail = |problem| CycleNumberError {
            input: input.to_owned(),
            problem,
        };
        if input.is_empty() || !input.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(fail(CycleNumberProblem::NotDigits));
        }
        let value = input
            .parse::<u32>()
            .map_err(|_| fail(CycleNumberProblem::TooLarge))?;
        let number = NonZeroU32::new(value).ok_or_else(|| fail(CycleNumberProblem::Zero))?;
        if input.starts_with('0') {
            return Err(fail(CycleNumberProblem::LeadingZero));
        }
        Ok(Self(number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(input: &str) -> CycleNumberProblem {
        input
            .parse::<CycleNumber>()
            .expect_err("invalid cycle number")
            .problem()
    }

    #[test]
    fn accepts_canonical_numbers_in_u32_range() {
        for (input, expected) in [("1", 1), ("42", 42), ("4294967295", u32::MAX)] {
            let number: CycleNumber = input.parse().expect("valid cycle number");
            assert_eq!(number.get(), expected);
            assert_eq!(number.to_string(), input);
        }
    }

    #[test]
    fn rejects_everything_else_with_a_reason() {
        for input in ["", "+1", "-1", " 1", "1 ", "1.0", "1e3", "Constructor", "٣"] {
            assert_eq!(problem(input), CycleNumberProblem::NotDigits, "{input:?}");
        }
        assert_eq!(problem("0"), CycleNumberProblem::Zero);
        assert_eq!(problem("00"), CycleNumberProblem::Zero);
        assert_eq!(problem("05"), CycleNumberProblem::LeadingZero);
        assert_eq!(problem("4294967296"), CycleNumberProblem::TooLarge);
        assert_eq!(problem("9007199254740992"), CycleNumberProblem::TooLarge);
    }

    #[test]
    fn error_messages_name_the_input_and_reason() {
        let message = |input: &str| {
            input
                .parse::<CycleNumber>()
                .expect_err("invalid cycle number")
                .to_string()
        };
        assert_eq!(message("next"), "\"next\" is not a cycle number");
        assert_eq!(
            message("0"),
            "\"0\" is not a cycle number: cycle numbers start at 1"
        );
        assert_eq!(
            message("4294967296"),
            "\"4294967296\" is not a cycle number: the largest cycle number is 4294967295"
        );
    }
}

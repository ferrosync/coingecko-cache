use snafu::Snafu;
use bigdecimal::{BigDecimal, ToPrimitive, Signed};
use num_bigint::BigInt;
use std::str::FromStr;
use serde::{Serialize, Deserialize, Deserializer, de};

#[derive(Serialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoundingMode {
    None,
    HalfUp,
    HalfUpOpposite,
    Down,
}

impl<'de> Deserialize<'de> for RoundingMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Snafu, Debug)]
pub enum RoundingModeParseError {
    #[snafu(display("Invalid rounding mode specified: '{}'", input))]
    InvalidFormat {
        input: String,
    }
}

impl FromStr for RoundingMode {
    type Err = RoundingModeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "")
            .as_str()
        {
            "halfup" => Ok(RoundingMode::HalfUp),
            "halfupopposite" => Ok(RoundingMode::HalfUpOpposite),
            "down" => Ok(RoundingMode::Down),
            "none" | "ignore" | "" => Ok(RoundingMode::None),
            _ => InvalidFormat { input: s.to_owned() }.fail()
        }
    }
}

pub trait RoundExt {
    fn with_rounding(&self, round_digits: i64, mode: RoundingMode) -> Self;
    fn with_rounding_min_digits(&self, round_digits: i64, min_digit: u8) -> Self;
}

// Source: `bigdecimal/lib.rs`
use num_integer::Integer;
#[inline(always)]
fn ten_to_the(pow: u64) -> BigInt {
    if pow < 20 {
        BigInt::from(10u64.pow(pow as u32))
    } else {
        let (half, rem) = pow.div_rem(&16);

        let mut x = ten_to_the(half);

        for _ in 0..4 {
            x = &x * &x;
        }

        if rem == 0 {
            x
        } else {
            x * ten_to_the(rem)
        }
    }
}

impl RoundExt for BigDecimal {
    fn with_rounding(&self, round_digits: i64, mode: RoundingMode) -> Self {
        match mode {
            RoundingMode::None =>
                self.clone(),
            RoundingMode::HalfUp =>
                self.round(round_digits),
            RoundingMode::HalfUpOpposite => {
                let (_, scale ) = self.as_bigint_and_exponent();
                let k = if scale <= 0 { 1 } else { self.digits() - scale as u64 };
                let max = BigDecimal::from(ten_to_the(k));
                &max - (&max - self).round(round_digits)
            },
            RoundingMode::Down =>
                self.with_scale(round_digits),
        }
    }

    // Modified from `bigdecimal/lib.rs` (MIT/Apache)
    fn with_rounding_min_digits(&self, round_digits: i64, min_digit: u8) -> Self {
        let (bigint, decimal_part_digits) = self.as_bigint_and_exponent();
        let need_to_round_digits = decimal_part_digits - round_digits;
        if round_digits >= 0 && need_to_round_digits <= 0 {
            return self.clone();
        }

        let mut number = bigint.to_i128().unwrap();
        if number < 0 {
            number = -number;
        }
        for _ in 0..(need_to_round_digits - 1) {
            number /= 10;
        }
        let digit: i8 = (number % 10) as i8;

        if digit < (min_digit as i8) {
            self.with_scale(round_digits)
        } else if bigint.is_negative() {
            self.with_scale(round_digits) - BigDecimal::new(BigInt::from(1), round_digits)
        } else {
            self.with_scale(round_digits) + BigDecimal::new(BigInt::from(1), round_digits)
        }
    }
}

#[cfg(test)]
mod tests {
    use bigdecimal::{BigDecimal};
    use std::str::FromStr;
    use crate::ext::bigdecimal::{RoundingMode, RoundExt};

    struct TestCase<'a> {
        input: &'a str,
        expect: &'a str,
        round_digits: i64,
        mode: RoundingMode,
    }

    impl<'a> TestCase<'a> {
        fn assert(&self) {
            let x = decimal(self.input);
            let expected = decimal(self.expect);
            let actual = x.with_rounding(self.round_digits, self.mode);
            assert_eq!(actual, expected)
        }
    }

    fn decimal(x: impl AsRef<str>) -> BigDecimal {
        BigDecimal::from_str(x.as_ref()).unwrap()
    }

    fn test_case_opposites_should_sum_total(dom: &BigDecimal) {
        let max = decimal("100");
        let altdom = &max - dom;

        let dom_rounded = dom.with_rounding(2, RoundingMode::HalfUp);
        let altdom_rounded = altdom.with_rounding(2, RoundingMode::HalfUpOpposite);
        let sum = &dom_rounded + &altdom_rounded;
        assert_eq!(sum, max,
                   "Got {}, but expected {}: DOM ({} => {}), ALTDOM ({} => {})",
                   &sum, &max,
                   &dom, &dom_rounded,
                   &altdom, &altdom_rounded);
    }

    #[test]
    fn opposites_should_sum_total() {
        let mut x = BigDecimal::from(0);
        let limit = BigDecimal::from(1);
        let step = BigDecimal::from_str("0.0001").unwrap();
        while x <= limit {
            test_case_opposites_should_sum_total(&x);
            x += &step;
        }
    }

    #[test]
    fn should_round_half_up_correctly() {
        TestCase {
            input : "12.451",
            expect: "12.45",
            round_digits: 2,
            mode: RoundingMode::HalfUp,
        }.assert();

        TestCase {
            input : "12.455",
            expect: "12.46",
            round_digits: 2,
            mode: RoundingMode::HalfUp,
        }.assert();
    }

    #[test]
    fn should_round_down_correctly() {
        TestCase {
            input : "12.455",
            expect: "12.45",
            round_digits: 2,
            mode: RoundingMode::Down,
        }.assert();

        TestCase {
            input : "12.459",
            expect: "12.45",
            round_digits: 2,
            mode: RoundingMode::Down,
        }.assert();
    }

    #[test]
    fn should_deserialize() {
        let x: RoundingMode = serde_json::from_str(r#""HALF_UP_OPPOSITE""#).unwrap();
        assert_eq!(x, RoundingMode::HalfUpOpposite);

        let x: RoundingMode = serde_json::from_str(r#""haLFUP___OppOSITE""#).unwrap();
        assert_eq!(x, RoundingMode::HalfUpOpposite);
    }
}
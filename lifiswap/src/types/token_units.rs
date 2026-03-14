//! Token amount formatting and parsing.
//!
//! Converts between raw base-unit strings (e.g. `"1000000"`) and
//! human-readable decimal strings (e.g. `"1.0"`) based on token decimals.

/// Format a raw base-unit amount string into a human-readable decimal string.
///
/// Equivalent to `ethers.formatUnits` / `viem.formatUnits`.
///
/// # Example
///
/// ```
/// use lifiswap::types::token_units::format_units;
///
/// assert_eq!(format_units("1000000", 6), "1.0");
/// assert_eq!(format_units("1000000000000000000", 18), "1.0");
/// assert_eq!(format_units("500000", 6), "0.5");
/// assert_eq!(format_units("0", 18), "0.0");
/// ```
#[must_use]
pub fn format_units(amount: &str, decimals: u8) -> String {
    let amount = amount.trim();
    if amount.is_empty() || amount == "0" {
        return "0.0".to_owned();
    }

    let negative = amount.starts_with('-');
    let digits = if negative { &amount[1..] } else { amount };

    let dec = decimals as usize;

    if dec == 0 {
        return if negative {
            format!("-{digits}.0")
        } else {
            format!("{digits}.0")
        };
    }

    let padded = if digits.len() <= dec {
        format!("{:0>width$}", digits, width = dec + 1)
    } else {
        digits.to_owned()
    };

    let split_at = padded.len() - dec;
    let integer = &padded[..split_at];
    let fraction = padded[split_at..].trim_end_matches('0');
    let fraction = if fraction.is_empty() { "0" } else { fraction };

    if negative {
        format!("-{integer}.{fraction}")
    } else {
        format!("{integer}.{fraction}")
    }
}

/// Parse a human-readable decimal string into a raw base-unit amount string.
///
/// Equivalent to `ethers.parseUnits` / `viem.parseUnits`.
///
/// # Example
///
/// ```
/// use lifiswap::types::token_units::parse_units;
///
/// assert_eq!(parse_units("1.0", 6).unwrap(), "1000000");
/// assert_eq!(parse_units("1.0", 18).unwrap(), "1000000000000000000");
/// assert_eq!(parse_units("0.5", 6).unwrap(), "500000");
/// assert_eq!(parse_units("100", 6).unwrap(), "100000000");
/// ```
///
/// # Errors
///
/// Returns `None` if the input contains invalid characters.
#[must_use]
pub fn parse_units(amount: &str, decimals: u8) -> Option<String> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Some("0".to_owned());
    }

    let negative = amount.starts_with('-');
    let abs = if negative { &amount[1..] } else { amount };
    let dec = decimals as usize;

    let (integer, fraction) = if let Some(dot_pos) = abs.find('.') {
        let int_part = &abs[..dot_pos];
        let frac_part = &abs[dot_pos + 1..];
        (int_part, frac_part)
    } else {
        (abs, "")
    };

    if !integer.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let frac_trimmed = if fraction.len() > dec {
        &fraction[..dec]
    } else {
        fraction
    };
    let frac_padded = format!("{frac_trimmed:0<dec$}");

    let raw = format!("{integer}{frac_padded}");
    let raw = raw.trim_start_matches('0');
    let raw = if raw.is_empty() { "0" } else { raw };

    if negative && raw != "0" {
        Some(format!("-{raw}"))
    } else {
        Some(raw.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_standard_cases() {
        assert_eq!(format_units("1000000", 6), "1.0");
        assert_eq!(format_units("1000000000000000000", 18), "1.0");
        assert_eq!(format_units("500000", 6), "0.5");
        assert_eq!(format_units("123456789", 6), "123.456789");
    }

    #[test]
    fn format_zero() {
        assert_eq!(format_units("0", 18), "0.0");
        assert_eq!(format_units("0", 0), "0.0");
        assert_eq!(format_units("", 6), "0.0");
    }

    #[test]
    fn format_no_decimals() {
        assert_eq!(format_units("42", 0), "42.0");
    }

    #[test]
    fn format_small_amount() {
        assert_eq!(format_units("1", 18), "0.000000000000000001");
    }

    #[test]
    fn parse_standard_cases() {
        assert_eq!(parse_units("1.0", 6).unwrap(), "1000000");
        assert_eq!(parse_units("1.0", 18).unwrap(), "1000000000000000000");
        assert_eq!(parse_units("0.5", 6).unwrap(), "500000");
    }

    #[test]
    fn parse_integer_input() {
        assert_eq!(parse_units("100", 6).unwrap(), "100000000");
    }

    #[test]
    fn parse_zero() {
        assert_eq!(parse_units("0", 18).unwrap(), "0");
        assert_eq!(parse_units("0.0", 6).unwrap(), "0");
    }

    #[test]
    fn roundtrip() {
        let original = "12345678901234567890";
        let formatted = format_units(original, 18);
        let parsed = parse_units(&formatted, 18).unwrap();
        assert_eq!(parsed, original);
    }
}

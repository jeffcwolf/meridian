//! Parsing and display-formatting of financial amounts. Facts are stored as
//! full-currency-unit strings (possibly with a decimal part); the UI shows them
//! grouped, in millions.

/// Parse a raw XBRL-JSON numeric string (which may carry a decimal part or sign)
/// to an integer amount, truncating any fractional part.
pub(crate) fn parse_amount(raw: &str) -> Option<i128> {
    let raw = raw.trim();
    let is_negative = raw.starts_with('-');
    let integer_part = raw
        .trim_start_matches(['-', '+'])
        .split('.')
        .next()
        .unwrap_or("");
    let digits: String = integer_part
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    let magnitude: i128 = digits.parse().ok()?;
    Some(if is_negative { -magnitude } else { magnitude })
}

/// Format a full-currency-unit amount as a thousands-grouped figure in millions,
/// e.g. `77_769_000_000` -> `"77,769"`.
pub(crate) fn fmt_millions(amount: i128) -> String {
    let millions = amount / 1_000_000;
    let grouped = group_thousands(millions.abs());
    if millions < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Group a non-negative integer with thousands separators: `1797062` -> `"1,797,062"`.
fn group_thousands(n: i128) -> String {
    let digits = n.abs().to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in digits.char_indices() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_amount_reads_plain_integers() {
        assert_eq!(parse_amount("77769000000"), Some(77_769_000_000));
    }

    #[test]
    fn parse_amount_truncates_decimals_without_rescaling() {
        // Some issuers emit "…000.0"; the fractional part must be dropped, not
        // folded into the integer (which would scale the value 10x).
        assert_eq!(parse_amount("1837081000000.0"), Some(1_837_081_000_000));
    }

    #[test]
    fn parse_amount_handles_leading_sign() {
        assert_eq!(parse_amount("-4409000000"), Some(-4_409_000_000));
    }

    #[test]
    fn parse_amount_rejects_non_numeric() {
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("n/a"), None);
    }

    #[test]
    fn fmt_millions_groups_and_signs() {
        assert_eq!(fmt_millions(77_769_000_000), "77,769");
        assert_eq!(fmt_millions(1_797_062_000_000), "1,797,062");
        assert_eq!(fmt_millions(-4_409_000_000), "-4,409");
        assert_eq!(fmt_millions(0), "0");
    }
}

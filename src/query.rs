//! Parsing of URL query strings shared by the comparator page and the export
//! handlers. Both receive repeated `id=` parameters (which `ParamsMap` collapses
//! to a single value), so they parse the raw string themselves.

/// Parse every repeated `id=` value from a raw query string.
pub(crate) fn parse_ids(query: &str) -> Vec<i64> {
    query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|kv| kv.strip_prefix("id="))
        .filter_map(|v| v.parse::<i64>().ok())
        .collect()
}

/// Parse a single `key=value` parameter, if present and non-empty.
pub(crate) fn parse_param(query: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    query
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(&prefix))
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repeated_ids_ignoring_other_params() {
        assert_eq!(parse_ids("?id=1&fy=2023&id=4&base=EUR"), vec![1, 4]);
    }

    #[test]
    fn parses_named_param_and_treats_empty_as_absent() {
        let query = "id=1&fy=2023&base=";
        assert_eq!(parse_param(query, "fy"), Some("2023".to_string()));
        assert_eq!(parse_param(query, "base"), None);
        assert_eq!(parse_param(query, "missing"), None);
    }
}

use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;
use std::path::PathBuf;

fn get_archive_name(dt: &Option<DateTime<FixedOffset>>) -> String {
    dt.map(|dt| dt.naive_utc().format("%Y-%m").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn classify_emails(
    list: Vec<(PathBuf, Option<DateTime<FixedOffset>>)>,
) -> HashMap<String, Vec<PathBuf>> {
    let mut map = HashMap::new();
    for (path, dt) in list {
        map.entry(get_archive_name(&dt))
            .or_insert_with(Vec::new)
            .push(path);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::DateTime;

    #[test]
    fn test_get_archive_name() {
        fn assert_name(time: &str, expected: &str) {
            let dt = Some(DateTime::parse_from_rfc3339(time).unwrap());
            assert_eq!(get_archive_name(&dt), expected);
        }

        assert_name("2017-06-30T20:00:00+04:00", "2017-06");
        assert_name("2017-06-30T20:00:00+00:00", "2017-06");
        assert_name("2017-06-30T20:00:00-04:00", "2017-07");

        assert_name("2017-07-01T03:59:59+04:00", "2017-06");
        assert_name("2017-07-01T03:59:59+00:00", "2017-07");
        assert_name("2017-07-01T03:59:59-04:00", "2017-07");

        assert_eq!(get_archive_name(&None), "unknown");
    }
}

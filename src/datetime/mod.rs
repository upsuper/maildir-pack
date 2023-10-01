mod parser;

use chrono::{DateTime, FixedOffset};
use combine::Parser;

/// Parses a data and time string used in Internet Message Format based on what
/// specified in RFC 5322 section 3.3.
///
/// Different from `DateTime::parse_from_rfc2822`, this in addition allows some
/// patterns which are not supported by that function, specifically:
/// * using single digit for hour / minute / second,
/// * support comment, and
/// * treating `-0000` as `+0000`.
///
/// Also this uses a byte slice which is more general than a str.
///
/// Note: to simplify the implementation, multi-line value handling defined in
/// IMF is ignored. Whitespace, tab, carriage return, and newline are handled
/// the same way. This function only accepts a complete datetime string.
pub fn parse_datetime(s: &[u8]) -> Option<DateTime<FixedOffset>> {
    match parser::date_time().parse(s) {
        Ok((dt, b"")) => Some(dt),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, TimeZone};

    #[test]
    fn test_parsed() {
        let utc = FixedOffset::east_opt(0).unwrap();
        let edt = FixedOffset::east_opt(-4 * 3600).unwrap();
        let mst = FixedOffset::east_opt(-7 * 3600).unwrap();
        let testcases: Vec<(&[u8], _)> = vec![
            (
                b"Wed, 18 Feb 2015 23:16:09 +0000",
                utc.with_ymd_and_hms(2015, 2, 18, 23, 16, 9).unwrap(),
            ),
            (
                b"Wed, 18 Feb 2015 23:59:60 -0400",
                edt.from_local_datetime(
                    &NaiveDate::from_ymd_opt(2015, 2, 18)
                        .unwrap()
                        .and_hms_milli_opt(23, 59, 59, 1_000)
                        .unwrap(),
                )
                .unwrap(),
            ),
            (
                b"Wed, 18 Feb 2015 23:59:59 EDT",
                edt.with_ymd_and_hms(2015, 2, 18, 23, 59, 59).unwrap(),
            ),
            (
                b"Thu, 29 Sep 2016 23:18:26 +0000",
                utc.with_ymd_and_hms(2016, 9, 29, 23, 18, 26).unwrap(),
            ),
            (
                b"Tue, 11 Jul 2017 18:30:33 +0000 (UTC)",
                utc.with_ymd_and_hms(2017, 7, 11, 18, 30, 33).unwrap(),
            ),
            (
                b"Sat, 01 Oct 2016 14:47:20 -0000",
                utc.with_ymd_and_hms(2016, 10, 1, 14, 47, 20).unwrap(),
            ),
            (
                b"Fri, 9 Nov 2007  1:10:02 -0700 (MST)",
                mst.with_ymd_and_hms(2007, 11, 9, 1, 10, 2).unwrap(),
            ),
        ];
        for (s, dt) in testcases {
            assert_eq!(parse_datetime(s), Some(dt));
        }
    }

    #[test]
    fn test_not_parsed() {
        let testcases: &[&[u8]] = &[b"Tue, 18 Feb 2015 23:16:09 +0000"];
        for s in testcases {
            assert_eq!(parse_datetime(s), None);
        }
    }
}

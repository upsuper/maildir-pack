use args::Args;
use chrono::{DateTime, FixedOffset};
use rayon::prelude::*;
use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use utils;

fn normalize_datetime(mut dt: &str) -> Cow<str> {
    // Trailing commentary timezone info is not recognized.
    if dt.ends_with(")") {
        if let Some(pos) = dt.rfind("(") {
            dt = &dt[..pos];
        }
    }
    // Trim whitespaces.
    dt = dt.trim();
    // -0000 timezone cannot be parsed. Let's just treat it as +0000.
    if dt.ends_with("-0000") {
        Cow::Owned(format!("{}+0000", &dt[..dt.len() - 5]))
    } else {
        Cow::Borrowed(dt)
    }
}

fn get_datetime_from_email(file: &Path) -> io::Result<Option<DateTime<FixedOffset>>> {
    const DATE_HEADER: &str = "Date: ";
    let reader = BufReader::new(File::open(file)?);
    for line in reader.lines() {
        let line = line?;
        if !line.starts_with(DATE_HEADER) {
            continue;
        }
        let dt_str = normalize_datetime(&line[DATE_HEADER.len()..]);
        if let Ok(dt) = DateTime::parse_from_rfc2822(&dt_str) {
            return Ok(Some(dt));
        }
    }
    Ok(None)
}

pub fn list_emails(args: &Args)
    -> io::Result<Vec<(PathBuf, Option<DateTime<FixedOffset>>)>>
{
    let mut files = vec![];
    for entry in fs::read_dir(&args.maildir.join("new"))? {
        files.push(entry?.path());
    }

    // There is no email, just return.
    if files.is_empty() {
        return Ok(vec![]);
    }

    let progress = utils::create_progress_bar(args, files.len());
    let result = files.into_par_iter().enumerate().map(|(i, path)| {
        let dt = get_datetime_from_email(&path).unwrap_or(None);
        if i % 128 == 127 {
            progress.inc(128);
        }
        (path, dt)
    }).collect();
    progress.finish_and_clear();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_datetime() {
        assert_eq!(normalize_datetime("Thu, 29 Sep 2016 23:18:26 +0000"),
                   "Thu, 29 Sep 2016 23:18:26 +0000");
        assert_eq!(normalize_datetime("Tue, 11 Jul 2017 18:30:33 +0000 (UTC)"),
                   "Tue, 11 Jul 2017 18:30:33 +0000");
        assert_eq!(normalize_datetime("Sat, 01 Oct 2016 14:47:20 -0000"),
                   "Sat, 01 Oct 2016 14:47:20 +0000");
    }
}

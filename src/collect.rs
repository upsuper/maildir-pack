use crate::args::Args;
use crate::datetime::parse_datetime;
use crate::utils;
use chrono::{DateTime, FixedOffset};
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Whether the given byte is a WSP as defined in RFC 5234 Appendix B.1
/// https://tools.ietf.org/html/rfc5234#appendix-B.1
fn is_wsp(b: u8) -> bool {
    b == 0x20 || b == 0x09
}

fn get_datetime_from_email(file: &Path) -> io::Result<Option<DateTime<FixedOffset>>> {
    const DATE_HEADER: &[u8] = b"date: ";
    let reader = BufReader::new(File::open(file)?);
    let mut date: Option<Vec<u8>> = None;
    for line in reader.split(b'\n') {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if date.is_some() {
            // Line breaks can be folded with whitespaces.
            if !is_wsp(line[0]) {
                break;
            }
            date.as_mut().unwrap().extend(line);
        } else {
            if line.len() <= DATE_HEADER.len()
                || !line[..DATE_HEADER.len()].eq_ignore_ascii_case(DATE_HEADER)
            {
                continue;
            }
            date = Some(line[DATE_HEADER.len()..].to_vec());
        }
    }
    Ok(date.as_ref().and_then(|dt| parse_datetime(dt)))
}

pub fn list_emails(args: &Args) -> io::Result<Vec<(PathBuf, Option<DateTime<FixedOffset>>)>> {
    let mut files = vec![];
    for entry in fs::read_dir(&args.maildir.join("new"))? {
        files.push(entry?.path());
    }

    // There is no email, just return.
    if files.is_empty() {
        return Ok(vec![]);
    }

    let progress = utils::create_progress_bar(args, files.len());
    let result = files
        .into_par_iter()
        .enumerate()
        .map(|(i, path)| {
            let dt = get_datetime_from_email(&path).unwrap_or(None);
            if i % 128 == 127 {
                progress.inc(128);
            }
            (path, dt)
        }).collect();
    progress.finish_and_clear();

    Ok(result)
}

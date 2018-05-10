use args::Args;
use rayon::prelude::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::{self, Archive as TarArchive, Builder as TarBuilder};
use utils;
use verify::{HashResult, StreamHasher};
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

fn get_file_name(path: &Path) -> &OsStr {
    path.file_name().expect("Unexpected path")
}

fn fill_archive_from(
    src: File,
    builder: &mut TarBuilder<impl Write>,
    files: &mut HashMap<OsString, HashResult>,
) -> io::Result<()> {
    let xz_reader = XzDecoder::new(src);
    let mut tar_archive = TarArchive::new(xz_reader);
    for entry in tar_archive.entries()? {
        let entry = entry?;
        // We have to clone the header, otherwise we cannot feed entry
        // to builder.append(). See alexcrichton/tar-rs#122.
        let header = entry.header().clone();
        let file_name = get_file_name(&header.path()?).to_os_string();
        let mut hasher = StreamHasher::new(entry);
        builder.append(&header, &mut hasher)?;
        // Add the path to the files map.
        files.insert(file_name, hasher.get_result());
    }

    Ok(())
}

#[cfg(unix)]
fn set_archive_permission(file: &File) -> io::Result<()> {
    let mut perms = file.metadata()?.permissions();
    let mode = (perms.mode() & !0o777) | 0o600;
    perms.set_mode(mode);
    file.set_permissions(perms)
}

fn do_archive(args: &Args, name: &str, emails: Vec<PathBuf>) -> io::Result<()> {
    let archive_name = format!("{}.tar.xz", name);
    let archive_path = args.packed_dir.join(&archive_name);

    let tmp_path = args.packed_dir.join(format!("{}.tmp", &archive_name));
    let tmp_file = File::create(&tmp_path)?;
    #[cfg(unix)]
    set_archive_permission(&tmp_file)?;

    let xz_writer = XzEncoder::new(tmp_file, 9);
    let mut tar_builder = TarBuilder::new(xz_writer);
    tar_builder.mode(tar::HeaderMode::Deterministic);

    // Fill files from existing archive and backup it.
    let mut existing_files = HashMap::new();
    if let Ok(file) = File::open(&archive_path) {
        fill_archive_from(file, &mut tar_builder, &mut existing_files)?;
        let backup_path = args.packed_dir.join(format!("{}.bak", archive_name));
        fs::rename(&archive_path, &backup_path)?;
    }

    // Adding emails to the archive.
    let existing_files = existing_files;
    for email in emails.iter() {
        let file_name = get_file_name(email);
        let mut file = File::open(email)?;
        if let Some(expected_hash) = existing_files.get(file_name) {
            // The file exists, let's check whether the hash matches.
            let mut hasher = StreamHasher::new(file);
            let mut buf = [0; 4096];
            while let Ok(size) = hasher.read(&mut buf) {
                if size == 0 {
                    break;
                }
            }
            let hash = hasher.get_result();
            if expected_hash[..] != hash[..] {
                eprintln!(
                    "Warning: {:?} exists in the archive \
                     but has different content",
                    file_name
                );
            }
        } else {
            tar_builder.append_file(file_name, &mut file)?;
        }
    }

    // Close the archive and move it to the destination.
    drop(tar_builder.into_inner()?.finish()?);
    fs::rename(&tmp_path, &archive_path)?;

    // Remove the archived emails.
    emails
        .par_iter()
        .for_each(|email| fs::remove_file(email).unwrap());

    Ok(())
}

pub fn archive_emails(args: &Args, map: HashMap<String, Vec<PathBuf>>) {
    let progress = utils::create_progress_bar(args, map.len());
    progress.tick();
    map.into_par_iter().for_each(|(name, emails)| {
        do_archive(args, &name, emails).unwrap();
        progress.inc(1);
    });
    progress.finish_and_clear();
}

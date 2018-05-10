#[macro_use]
extern crate lazy_static;
extern crate leak;
extern crate sha2;
extern crate tar;
extern crate tempdir;
extern crate xz2;

use leak::Leak;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::mem;
use std::ops::Deref;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive as TarArchive;
use tempdir::TempDir;
use xz2::read::XzDecoder;

type HashResult = [u8; 32];

const ARCHIVE_SUFFIX: &'static str = ".tar.xz";
const BACKUP_SUFFIX: &'static str = ".tar.xz.bak";

lazy_static! {
    static ref KEEP_TEST_DIR: bool = { env::var("KEEP_TEST_DIR").is_ok() };
    static ref BIN_PATH: PathBuf = {
        let exe = env::current_exe().unwrap();
        let mut path = exe.parent().unwrap().to_path_buf();
        path.set_file_name(env!("CARGO_PKG_NAME"));
        path.set_extension(env::consts::EXE_EXTENSION);
        path
    };
    static ref EMAILS_PATH: PathBuf = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut path = Path::new(manifest_dir).to_path_buf();
        path.push("tests");
        path.push("emails");
        path
    };
    static ref ALL_EMAILS: HashMap<&'static str, Vec<&'static Path>> = { list_emails().unwrap() };
    static ref EMAIL_HASHS: HashMap<&'static Path, HashResult> = { hash_emails().unwrap() };
}

fn list_emails() -> io::Result<HashMap<&'static str, Vec<&'static Path>>> {
    fn should_skip(path: &Path) -> bool {
        let file_name = path.file_name().unwrap();
        let file_name = file_name.to_str().unwrap();
        file_name.starts_with('.')
    }

    let mut result = HashMap::new();
    for source in EMAILS_PATH.read_dir()? {
        let source = source?.path();
        if !source.is_dir() {
            continue;
        }
        for dir in source.read_dir()? {
            let dir = dir?.path();
            if !dir.is_dir() {
                continue;
            }
            let name = dir.file_name().unwrap();
            let name = name.to_str().unwrap().to_string();
            let name = name.into_boxed_str().leak();
            let entry = result.entry(name).or_insert(vec![]);
            for item in dir.read_dir()? {
                let item = item?.path().into_boxed_path().leak();
                if !should_skip(item) {
                    entry.push(item);
                }
            }
        }
    }
    Ok(result)
}

fn hash_content(mut reader: impl Read) -> io::Result<HashResult> {
    let mut hasher = Sha256::new();
    let mut buf: [u8; 4096] = unsafe { mem::uninitialized() };
    loop {
        let len = reader.read(&mut buf)?;
        if len == 0 {
            break;
        }
        hasher.input(&buf[..len]);
    }
    let mut result = [0; 32];
    result.copy_from_slice(hasher.result().as_slice());
    Ok(result)
}

fn hash_emails() -> io::Result<HashMap<&'static Path, HashResult>> {
    let mut result = HashMap::new();
    for &email in ALL_EMAILS.values().flat_map(|l| l.iter()) {
        let hash = hash_content(File::open(email)?)?;
        result.insert(email, hash);
    }
    Ok(result)
}

struct TempMaildir {
    name: &'static str,
    tmp_dir: Option<TempDir>,
    new_dir: PathBuf,
    packed_dir: PathBuf,
}

impl TempMaildir {
    fn new(name: &'static str) -> io::Result<Self> {
        let dir_name = format!("{}-{}", env!("CARGO_PKG_NAME"), name);
        let tmp_dir = TempDir::new(&dir_name)?;
        let new_dir = tmp_dir.path().join("new");
        let packed_dir = tmp_dir.path().join("packed");
        // Create maildir structure. Since we currently only use the new
        // directory, we only create it. If we extend support to cur at
        // some point, we should create that as well.
        fs::create_dir(&new_dir)?;
        Ok(TempMaildir {
            name,
            tmp_dir: Some(tmp_dir),
            new_dir,
            packed_dir,
        })
    }

    fn path(&self) -> &Path {
        self.tmp_dir.as_ref().unwrap().path()
    }

    fn fill_maildir(&self, emails: impl Iterator<Item = impl AsRef<Path>>) -> io::Result<()> {
        for email in emails {
            let email = email.as_ref();
            fs::copy(email, self.new_dir.join(email.file_name().unwrap()))?;
        }
        Ok(())
    }

    fn execute_packing(&self) {
        let result = Command::new(&*BIN_PATH)
            .arg("--quiet")
            .arg(self.path())
            .status()
            .expect("Failed to execute");
        assert!(result.success());
    }
}

impl Drop for TempMaildir {
    fn drop(&mut self) {
        let tmp_dir = self.tmp_dir.take().unwrap();
        if !*KEEP_TEST_DIR {
            tmp_dir.close().unwrap();
        } else {
            let path = tmp_dir.into_path();
            eprintln!("{}: {:?}", self.name, path);
        }
    }
}

fn generate_email_set(
    iter: impl Iterator<Item = impl Deref<Target = &'static Path>>,
) -> HashSet<&'static Path> {
    iter.map(|email| *email.deref()).collect()
}

fn generate_expected_result(
    email_set: &HashSet<&'static Path>,
) -> HashMap<&'static str, HashMap<&'static OsStr, HashResult>> {
    ALL_EMAILS
        .iter()
        .filter_map(|(&archive, emails)| {
            let expected_content = emails
                .iter()
                .filter(|&email| email_set.contains(email))
                .map(|email| {
                    let file_name = email.file_name().unwrap();
                    let hash = EMAIL_HASHS[email];
                    (file_name, hash)
                })
                .collect::<HashMap<_, _>>();
            if !expected_content.is_empty() {
                Some((archive, expected_content))
            } else {
                None
            }
        })
        .collect()
}

fn join_names<'a>(mut iter: impl Iterator<Item = &'a str>) -> String {
    let mut result = String::new();
    if let Some(first) = iter.next() {
        result.push_str(first);
        for name in iter {
            result.push_str(", ");
            result.push_str(name);
        }
    }
    result
}

fn get_name_with_suffix<'a>(file_name: &'a str, suffix: &str) -> Option<&'a str> {
    if file_name.ends_with(suffix) {
        Some(&file_name[..file_name.len() - suffix.len()])
    } else {
        None
    }
}

fn check_packed(
    maildir: &TempMaildir,
    mut expected: HashMap<&str, HashMap<&OsStr, HashResult>>,
    mut expected_backup: HashMap<&str, HashResult>,
) -> io::Result<()> {
    for archive in fs::read_dir(&maildir.packed_dir)? {
        let archive = archive?.path();
        let archive_name = archive.file_name().unwrap().to_str().unwrap();
        let report_unexpected_file =
            || -> ! { panic!("Unexpected file {} in maildir/packed", archive_name) };
        if let Some(key) = get_name_with_suffix(archive_name, ARCHIVE_SUFFIX) {
            // Retrieve the expected content of the archive.
            let mut expected_content = match expected.remove(key) {
                Some(content) => content,
                None => report_unexpected_file(),
            };
            let file = File::open(&archive)?;
            // Check the permission.
            #[cfg(unix)]
            assert_eq!(
                file.metadata()?.permissions().mode() & 0o777,
                0o600,
                "Archive file should use mode 0o600"
            );
            // Read the archive and check the content.
            let xz_reader = XzDecoder::new(file);
            let mut tar_archive = TarArchive::new(xz_reader);
            for entry in tar_archive.entries()? {
                let entry = entry?;
                let file_name = entry.header().path()?.into_owned();
                let file_name = file_name.as_os_str();
                // Retrieve the expected hash.
                let expected_hash = match expected_content.remove(file_name) {
                    Some(hash) => hash,
                    None => panic!(
                        "Unexpected file {:?} in archive {}",
                        file_name, archive_name
                    ),
                };
                // Calculate actual hash of the content.
                let hash = hash_content(entry)?;
                assert_eq!(
                    hash, expected_hash,
                    "Content of file {:?} in archive {} mismatches",
                    file_name, archive_name
                );
            }
            // Check that no file left.
            if expected_content.len() > 0 {
                let files = join_names(expected_content.keys().map(|name| name.to_str().unwrap()));
                panic!("Files not found in archive {}: {}", archive_name, files);
            }
            continue;
        }
        if let Some(key) = get_name_with_suffix(archive_name, BACKUP_SUFFIX) {
            let expected_hash = match expected_backup.remove(key) {
                Some(hash) => hash,
                None => report_unexpected_file(),
            };
            let hash = hash_content(File::open(&archive)?)?;
            assert_eq!(
                hash, expected_hash,
                "Content of backup file {:?} mismatches",
                archive_name
            );
            continue;
        }
        report_unexpected_file();
    }
    // Check that all archives are created.
    if expected.len() > 0 {
        let archives = join_names(expected.keys().map(|&name| name));
        panic!("Archives not found in maildir/packed: {}", archives);
    }
    // Check that all backup are created.
    if expected_backup.len() > 0 {
        let backups = join_names(expected_backup.keys().map(|&name| name));
        panic!("Backups not found in maildir/packed: {}", backups);
    }
    Ok(())
}

fn check_maildir(
    maildir: &TempMaildir,
    mut expected: HashMap<&OsStr, HashResult>,
) -> io::Result<()> {
    for file in fs::read_dir(&maildir.new_dir)? {
        let file = file?.path();
        let file_name = file.file_name().unwrap();
        // Retrieve the expected hash.
        let expected_hash = match expected.remove(file_name) {
            Some(hash) => hash,
            None => panic!("Unexpected file {:?} in maildir/new", file_name),
        };
        // Calculate actual hash of the content.
        let hash = hash_content(File::open(&file)?)?;
        assert_eq!(
            hash, expected_hash,
            "Content of file {:?} in maildir/new mismatches",
            file_name
        );
    }
    // Check that no file left.
    if expected.len() > 0 {
        let files = join_names(expected.keys().map(|&name| name.to_str().unwrap()));
        panic!("Files not found in maildir/new: {}", files);
    }
    Ok(())
}

#[test]
fn basic_packing() {
    let maildir = TempMaildir::new("basic_packing").unwrap();
    // Copy all emails into the new dir.
    let emails = generate_email_set(ALL_EMAILS.values().flat_map(|l| l.iter()));
    maildir.fill_maildir(emails.iter()).unwrap();
    // Pack the maildir.
    maildir.execute_packing();
    // Check the result.
    let expected = generate_expected_result(&emails);
    check_packed(&maildir, expected, HashMap::new()).unwrap();
    // Check that maildir is empty now.
    check_maildir(&maildir, HashMap::new()).unwrap();
}

#[test]
fn incremental_packing() {
    let maildir = TempMaildir::new("incremental_packing").unwrap();
    // Generate test sets.
    let archives: Vec<_> = ALL_EMAILS
        .iter()
        .filter(|&(_, emails)| emails.len() >= 2)
        .collect();
    let initial_set = generate_email_set(
        archives
            .iter()
            .flat_map(|&(_, emails)| emails[..emails.len() * 2 / 3].iter()),
    );
    let second_set = generate_email_set(
        archives
            .iter()
            .flat_map(|&(_, emails)| emails[emails.len() / 3..].iter()),
    );
    assert!(!initial_set.is_empty());
    assert!(!second_set.is_empty());
    assert!(!initial_set.is_disjoint(&second_set));
    assert!(!initial_set.is_superset(&second_set));

    /* Initial packing */
    maildir.fill_maildir(initial_set.iter()).unwrap();
    maildir.execute_packing();
    let expected = generate_expected_result(&initial_set);
    check_packed(&maildir, expected, HashMap::new()).unwrap();
    check_maildir(&maildir, HashMap::new()).unwrap();

    /* Collect current content of packed */
    let expected_backup = archives
        .iter()
        .map(|&(&archive, _)| {
            let file_name = format!("{}{}", archive, ARCHIVE_SUFFIX);
            let path = maildir.packed_dir.join(file_name);
            let file = File::open(path).unwrap();
            let hash = hash_content(file).unwrap();
            (archive, hash)
        })
        .collect();

    /* Second packing */
    maildir.fill_maildir(second_set.iter()).unwrap();
    maildir.execute_packing();
    let merged = second_set.union(&initial_set).map(|&email| email).collect();
    let expected = generate_expected_result(&merged);
    check_packed(&maildir, expected, expected_backup).unwrap();
    check_maildir(&maildir, HashMap::new()).unwrap();
}

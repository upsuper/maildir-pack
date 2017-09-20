#[macro_use]
extern crate lazy_static;
extern crate sha2;
extern crate tar;
extern crate tempdir;
extern crate xz2;

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::mem;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive as TarArchive;
use tempdir::TempDir;
use xz2::read::XzDecoder;

type HashResult = [u8; 32];

const ARCHIVE_SUFFIX: &'static str = ".tar.xz";

lazy_static! {
    static ref KEEP_TEST_DIR: bool = {
        env::var("KEEP_TEST_DIR").is_ok()
    };
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
    static ref ALL_EMAILS: HashMap<String, Vec<PathBuf>> = {
        list_emails().unwrap()
    };
    static ref EMAIL_HASHS: HashMap<&'static Path, HashResult> = {
        hash_emails().unwrap()
    };
}

fn list_emails() -> io::Result<HashMap<String, Vec<PathBuf>>> {
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
            let entry = result.entry(name).or_insert(vec![]);
            for item in dir.read_dir()? {
                let item = item?.path();
                if !should_skip(&item) {
                    entry.push(item);
                }
            }
        }
    }
    Ok(result)
}

fn hash_content<R: Read>(mut reader: R) -> io::Result<HashResult> {
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
    for email in ALL_EMAILS.values().flat_map(|l| l.iter()) {
        let hash = hash_content(File::open(email)?)?;
        result.insert(email.as_path(), hash);
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
    fn path(&self) -> &Path {
        self.tmp_dir.as_ref().unwrap().path()
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

fn setup_maildir(name: &'static str) -> io::Result<TempMaildir> {
    let tmp_dir = TempDir::new(env!("CARGO_PKG_NAME"))?;
    let new_dir = tmp_dir.path().join("new");
    let packed_dir = tmp_dir.path().join("packed");
    // Create maildir structure. Since we currently only use the new
    // directory, we only create it. If we extend support to cur at
    // some point, we should create that as well.
    fs::create_dir(&new_dir)?;
    // Copy all emails into the new dir.
    for email in ALL_EMAILS.values().flat_map(|l| l.iter()) {
        fs::copy(email, new_dir.join(email.file_name().unwrap()))?;
    }
    Ok(TempMaildir {
        name,
        tmp_dir: Some(tmp_dir),
        new_dir,
        packed_dir,
    })
}

fn join_names<'a, I: Iterator<Item=&'a str>>(mut iter: I) -> String {
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

fn check_packed(maildir: &TempMaildir,
                mut expected: HashMap<&str, HashMap<&OsStr, HashResult>>)
    -> io::Result<()>
{
    for archive in fs::read_dir(&maildir.packed_dir)? {
        let archive = archive?.path();
        let archive_name = archive.file_name().unwrap().to_str().unwrap();
        let key_len = archive_name.len() - ARCHIVE_SUFFIX.len();
        let (key, suffix) = archive_name.split_at(key_len);
        assert_eq!(suffix, ARCHIVE_SUFFIX);
        // Retrieve the expected content of the archive.
        let mut expected_content = match expected.remove(key) {
            Some(content) => content,
            None => panic!("Unexpected file {} in maildir/packed",
                           archive_name),
        };
        // Read the archive and check the content.
        let file = File::open(&archive)?;
        let xz_reader = XzDecoder::new(file);
        let mut tar_archive = TarArchive::new(xz_reader);
        for entry in tar_archive.entries()? {
            let entry = entry?;
            let file_name = entry.header().path()?.into_owned();
            let file_name = file_name.as_os_str();
            // Retrieve the expected hash.
            let expected_hash = match expected_content.remove(file_name) {
                Some(hash) => hash,
                None => panic!("Unexpected file {:?} in archive {}",
                               file_name, archive_name),
            };
            // Calculate actual hash of the content.
            let hash = hash_content(entry)?;
            assert_eq!(hash, expected_hash,
                       "Content of file {:?} in archive {} mismatches",
                       file_name, archive_name);
        }
        // Check that no file left.
        if expected_content.len() > 0 {
            let files = join_names(expected_content.keys().map(|name| {
                name.to_str().unwrap()
            }));
            panic!("Files not found in archive {}: {}", archive_name, files);
        }
    }
    // Check that all archives are created.
    if expected.len() > 0 {
        let archives = join_names(expected.keys().map(|&name| name));
        panic!("Archives not found in maildir/packed: {}", archives);
    }
    Ok(())
}

fn check_maildir(maildir: &TempMaildir,
                 mut expected: HashMap<&OsStr, HashResult>)
    -> io::Result<()>
{
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
        assert_eq!(hash, expected_hash,
                   "Content of file {:?} in maildir/new mismatches",
                   file_name);
    }
    // Check that no file left.
    if expected.len() > 0 {
        let files = join_names(expected.keys().map(|&name| {
            name.to_str().unwrap()
        }));
        panic!("Files not found in maildir/new: {}", files);
    }
    Ok(())
}

#[test]
fn basic_packing() {
    let maildir = setup_maildir("basic_packing").unwrap();
    // Pack the maildir.
    let result = Command::new(&*BIN_PATH)
        .arg("--quiet")
        .arg(maildir.path())
        .status()
        .expect("Failed to execute");
    assert!(result.success());
    // Check the result.
    let expected = ALL_EMAILS.iter().map(|(archive, files)| {
        let archive = archive.as_str();
        let expected_content = files.iter().map(|path| {
            let file_name = path.file_name().unwrap();
            let path: &Path = &path;
            let hash = EMAIL_HASHS[path];
            (file_name, hash)
        }).collect();
        (archive, expected_content)
    }).collect();
    check_packed(&maildir, expected).unwrap();
    // Check that maildir is empty now.
    check_maildir(&maildir, HashMap::new()).unwrap();
}

//! Packing and hardened unpacking of registry packages.
//!
//! A package is a gzip tarball holding `veld-package.toml` and `contracts/**.veld`. Packing is
//! deterministic (sorted entries, zero mtimes, fixed modes and owners, fixed compression).
//! Unpacking never calls `tar`'s own extraction: entries are read into memory under strict
//! limits, validated, and only then written by this module.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use flate2::Compression;
use flate2::GzBuilder;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::{Builder, EntryType, Header};
use veld_project::packages::{CONTRACTS_DIR, MANIFEST_FILE};

use crate::error::{RegistryError, Result, io};
use crate::manifest::PackageManifest;

/// Limits applied while unpacking an untrusted archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    /// Largest allowed uncompressed/compressed ratio (checked once the output passes 1 MiB).
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 2000,
            max_entry_bytes: 8 << 20,
            max_total_bytes: 64 << 20,
            max_ratio: 100,
        }
    }
}

/// A packed archive and its SHA-256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    pub bytes: Vec<u8>,
    pub sha256: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn bad(msg: impl Into<String>) -> RegistryError {
    RegistryError::Tarball(msg.into())
}

/// Checks one archive path. Returns an error for anything that could escape the destination.
pub fn check_path(name: &str) -> Result<()> {
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    if trimmed.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(char::is_control)
    {
        return Err(bad(format!(
            "entry name `{}` is not allowed",
            name.escape_debug()
        )));
    }
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.ends_with('.') || seg.ends_with(' ') {
            return Err(bad(format!("entry name `{name}` is not allowed")));
        }
    }
    Ok(())
}

fn check_file_name(name: &str) -> Result<()> {
    check_path(name)?;
    let allowed = name == MANIFEST_FILE
        || (name.starts_with("contracts/") && name.ends_with(".veld") && !name.ends_with('/'));
    if allowed {
        Ok(())
    } else {
        Err(bad(format!(
            "entry `{name}` is outside contracts/ or is not a .veld file"
        )))
    }
}

/// Builds the package archive. `files` are `(path relative to the contracts dir, contents)`.
pub fn pack(manifest: &PackageManifest, files: &[(String, Vec<u8>)]) -> Result<Packed> {
    let mut entries: BTreeMap<String, &[u8]> = BTreeMap::new();
    for (rel, data) in files {
        let name = format!("{CONTRACTS_DIR}/{rel}");
        check_file_name(&name)?;
        if entries.insert(name.clone(), data).is_some() {
            return Err(bad(format!("duplicate file `{name}`")));
        }
    }
    let manifest_text = manifest.to_toml();
    entries.insert(MANIFEST_FILE.to_string(), manifest_text.as_bytes());

    let gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(9));
    let mut builder = Builder::new(gz);
    for (name, data) in &entries {
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        builder
            .append_data(&mut header, name, *data)
            .map_err(|e| bad(e.to_string()))?;
    }
    let gz = builder.into_inner().map_err(|e| bad(e.to_string()))?;
    let bytes = gz.finish().map_err(|e| bad(e.to_string()))?;
    Ok(Packed {
        sha256: sha256_hex(&bytes),
        bytes,
    })
}

/// Reads every entry of `archive` into memory (`name -> contents`), enforcing `limits`.
pub fn read_archive(archive: &[u8], limits: &Limits) -> Result<BTreeMap<String, Vec<u8>>> {
    let compressed = (archive.len() as u64).max(1);
    let decoder = GzDecoder::new(archive).take(limits.max_total_bytes.saturating_add(1 << 20));
    let mut tar = tar::Archive::new(decoder);
    let entries = tar
        .entries()
        .map_err(|e| bad(format!("not a gzip tarball: {e}")))?;

    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut seen_lower: BTreeSet<String> = BTreeSet::new();
    let mut count = 0usize;
    let mut total = 0u64;
    for entry in entries {
        let mut entry = entry.map_err(|e| bad(format!("corrupt archive: {e}")))?;
        count += 1;
        if count > limits.max_entries {
            return Err(bad(format!("more than {} entries", limits.max_entries)));
        }
        let kind = entry.header().entry_type();
        let path = std::str::from_utf8(&entry.path_bytes())
            .map_err(|_| bad("entry name is not valid UTF-8"))?
            .to_string();
        match kind {
            EntryType::Directory => {
                check_path(&path)?;
                let dir = path.trim_end_matches('/');
                if dir != CONTRACTS_DIR && !dir.starts_with("contracts/") {
                    return Err(bad(format!("directory `{path}` is outside contracts/")));
                }
                continue;
            }
            EntryType::Regular => {}
            other => {
                return Err(bad(format!(
                    "entry `{path}` has type {other:?}; only regular files and directories are allowed"
                )));
            }
        }
        check_file_name(&path)?;
        if !seen_lower.insert(path.to_lowercase()) {
            return Err(bad(format!("duplicate entry `{path}`")));
        }
        let declared = entry.header().size().map_err(|e| bad(e.to_string()))?;
        if declared > limits.max_entry_bytes {
            return Err(bad(format!(
                "entry `{path}` is larger than the per-file limit"
            )));
        }
        let mut data = Vec::new();
        (&mut entry)
            .take(limits.max_entry_bytes + 1)
            .read_to_end(&mut data)
            .map_err(|e| bad(format!("corrupt archive: {e}")))?;
        total += data.len() as u64;
        if data.len() as u64 > limits.max_entry_bytes || total > limits.max_total_bytes {
            return Err(bad("archive exceeds the total size limit"));
        }
        if total > (1 << 20) && total / compressed > limits.max_ratio {
            return Err(bad(format!(
                "compression ratio above {}:1 (possible decompression bomb)",
                limits.max_ratio
            )));
        }
        files.insert(path, data);
    }
    Ok(files)
}

/// Parses and checks the manifest against what the archive actually contains.
pub fn verify_contents(
    files: &BTreeMap<String, Vec<u8>>,
    name: &str,
    version: &str,
) -> Result<PackageManifest> {
    let text = files
        .get(MANIFEST_FILE)
        .ok_or_else(|| RegistryError::Manifest(format!("package has no {MANIFEST_FILE}")))?;
    let text = std::str::from_utf8(text)
        .map_err(|_| RegistryError::Manifest(format!("{MANIFEST_FILE} is not UTF-8")))?;
    let manifest = PackageManifest::parse(text)?;
    if manifest.name != name || manifest.version != version {
        return Err(RegistryError::Manifest(format!(
            "package is {}@{} but {name}@{version} was expected",
            manifest.name, manifest.version
        )));
    }
    for (path, want) in &manifest.files {
        let data = files.get(path).ok_or_else(|| {
            RegistryError::Manifest(format!("manifest lists `{path}` but the archive lacks it"))
        })?;
        let got = sha256_hex(data);
        if &got != want {
            return Err(RegistryError::Checksum {
                what: format!("{name}@{version} file `{path}`"),
                expected: want.clone(),
                actual: got,
            });
        }
    }
    if let Some(extra) = files
        .keys()
        .find(|k| k.as_str() != MANIFEST_FILE && !manifest.files.contains_key(*k))
    {
        return Err(RegistryError::Manifest(format!(
            "archive file `{extra}` is not listed in the manifest"
        )));
    }
    Ok(manifest)
}

/// Writes validated `files` under `dest` (which must not exist yet or be empty).
pub fn write_files(files: &BTreeMap<String, Vec<u8>>, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(io(dest))?;
    for (name, data) in files {
        check_file_name(name)?;
        let target = name
            .split('/')
            .fold(dest.to_path_buf(), |p, seg| p.join(seg));
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(io(dir))?;
        }
        std::fs::write(&target, data).map_err(io(&target))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_for(files: &[(String, Vec<u8>)]) -> PackageManifest {
        PackageManifest::new("@acme/lib", "1.0.0", Some("d"), files)
    }

    fn sample() -> Vec<(String, Vec<u8>)> {
        vec![
            ("b.veld".into(), b"model B {}".to_vec()),
            ("a/x.veld".into(), b"model X {}".to_vec()),
        ]
    }

    /// Builds an arbitrary (possibly hostile) gzip tarball.
    fn raw_archive(entries: &[(&str, EntryType, &[u8])]) -> Vec<u8> {
        let gz = GzBuilder::new().write(Vec::new(), Compression::fast());
        let mut b = Builder::new(gz);
        for (name, kind, data) in entries {
            let mut h = Header::new_gnu();
            h.set_entry_type(*kind);
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            // Bypass the safe path setter so hostile names reach the archive.
            let bytes = name.as_bytes();
            let field = &mut h.as_old_mut().name;
            field.fill(0);
            field[..bytes.len()].copy_from_slice(bytes);
            if matches!(kind, EntryType::Symlink | EntryType::Link) {
                let link = b"target";
                h.as_old_mut().linkname[..link.len()].copy_from_slice(link);
            }
            h.set_cksum();
            b.append(&h, *data).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap()
    }

    type RawEntry<'a> = (&'a str, EntryType, &'a [u8]);

    fn reg(name: &str) -> RawEntry<'_> {
        (name, EntryType::Regular, b"x")
    }

    #[test]
    fn pack_is_byte_identical_and_sorted() {
        let files = sample();
        let m = manifest_for(&files);
        let a = pack(&m, &files).unwrap();
        let mut reversed = files.clone();
        reversed.reverse();
        let b = pack(&m, &reversed).unwrap();
        assert_eq!(a.bytes, b.bytes);
        assert_eq!(a.sha256, sha256_hex(&a.bytes));
        let unpacked = read_archive(&a.bytes, &Limits::default()).unwrap();
        let names: Vec<&String> = unpacked.keys().collect();
        assert_eq!(
            names,
            [
                "contracts/a/x.veld",
                "contracts/b.veld",
                "veld-package.toml"
            ]
        );
    }

    #[test]
    fn pack_uses_zero_mtimes_and_fixed_modes() {
        let files = sample();
        let packed = pack(&manifest_for(&files), &files).unwrap();
        let gz = GzDecoder::new(packed.bytes.as_slice());
        let mut ar = tar::Archive::new(gz);
        for e in ar.entries().unwrap() {
            let e = e.unwrap();
            assert_eq!(e.header().mtime().unwrap(), 0);
            assert_eq!(e.header().mode().unwrap(), 0o644);
            assert_eq!(e.header().uid().unwrap(), 0);
        }
        assert_eq!(&packed.bytes[4..8], &[0, 0, 0, 0], "gzip mtime is zero");
    }

    #[test]
    fn round_trip_verifies_and_writes() {
        let files = sample();
        let m = manifest_for(&files);
        let packed = pack(&m, &files).unwrap();
        let read = read_archive(&packed.bytes, &Limits::default()).unwrap();
        let back = verify_contents(&read, "@acme/lib", "1.0.0").unwrap();
        assert_eq!(back, m);
        let tmp = tempfile::tempdir().unwrap();
        write_files(&read, &tmp.path().join("out")).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("out/contracts/a/x.veld")).unwrap();
        assert_eq!(text, "model X {}");
    }

    #[test]
    fn rejects_hostile_entries() {
        let cases: Vec<(&str, Vec<RawEntry>)> = vec![
            ("parent traversal", vec![reg("contracts/../../evil.veld")]),
            ("leading dotdot", vec![reg("../evil.veld")]),
            ("absolute", vec![reg("/etc/passwd")]),
            ("backslash", vec![reg("contracts\\evil.veld")]),
            ("drive letter", vec![reg("C:evil.veld")]),
            ("drive path", vec![reg("C:/evil.veld")]),
            ("outside contracts", vec![reg("src/evil.veld")]),
            ("not a veld file", vec![reg("contracts/evil.sh")]),
            ("empty segment", vec![reg("contracts//a.veld")]),
            ("dot segment", vec![reg("contracts/./a.veld")]),
            (
                "symlink",
                vec![("contracts/l.veld", EntryType::Symlink, b"")],
            ),
            ("hardlink", vec![("contracts/l.veld", EntryType::Link, b"")]),
            (
                "char device",
                vec![("contracts/d.veld", EntryType::Char, b"")],
            ),
            (
                "block device",
                vec![("contracts/d.veld", EntryType::Block, b"")],
            ),
            ("fifo", vec![("contracts/d.veld", EntryType::Fifo, b"")]),
            ("dir outside", vec![("etc/", EntryType::Directory, b"")]),
            (
                "duplicate",
                vec![reg("contracts/a.veld"), reg("contracts/a.veld")],
            ),
            (
                "case duplicate",
                vec![reg("contracts/a.veld"), reg("contracts/A.veld")],
            ),
        ];
        for (name, entries) in cases {
            let archive = raw_archive(&entries);
            assert!(
                matches!(
                    read_archive(&archive, &Limits::default()),
                    Err(RegistryError::Tarball(_))
                ),
                "case `{name}` must be rejected"
            );
        }
    }

    #[test]
    fn rejects_non_gzip() {
        assert!(read_archive(b"definitely not gzip", &Limits::default()).is_err());
    }

    #[test]
    fn enforces_entry_count_size_and_total_limits() {
        let many: Vec<(String, Vec<u8>)> = (0..10)
            .map(|i| (format!("f{i}.veld"), vec![b'a']))
            .collect();
        let packed = pack(&manifest_for(&many), &many).unwrap();
        let tight = Limits {
            max_entries: 5,
            ..Limits::default()
        };
        assert!(read_archive(&packed.bytes, &tight).is_err());

        let big = vec![("big.veld".to_string(), vec![b'a'; 2000])];
        let packed = pack(&manifest_for(&big), &big).unwrap();
        let per_entry = Limits {
            max_entry_bytes: 1000,
            ..Limits::default()
        };
        assert!(read_archive(&packed.bytes, &per_entry).is_err());
        let total = Limits {
            max_total_bytes: 1500,
            ..Limits::default()
        };
        assert!(read_archive(&packed.bytes, &total).is_err());
    }

    #[test]
    fn rejects_a_zip_bomb_like_ratio() {
        // 6 MiB of zeros compresses to a few KiB: far beyond a 100:1 ratio, yet below the
        // per-entry and total byte limits, so only the ratio check can stop it.
        let bomb = vec![("zeros.veld".to_string(), vec![0u8; 6 << 20])];
        let packed = pack(&manifest_for(&bomb), &bomb).unwrap();
        assert!(packed.bytes.len() < 100 << 10);
        let err = read_archive(&packed.bytes, &Limits::default()).unwrap_err();
        assert!(err.to_string().contains("ratio"), "{err}");
    }

    #[test]
    fn verify_detects_tampering_and_mismatches() {
        let files = sample();
        let packed = pack(&manifest_for(&files), &files).unwrap();
        let mut read = read_archive(&packed.bytes, &Limits::default()).unwrap();
        assert!(verify_contents(&read, "@acme/other", "1.0.0").is_err());
        assert!(verify_contents(&read, "@acme/lib", "2.0.0").is_err());

        let mut changed = read.clone();
        changed.insert("contracts/b.veld".into(), b"model Evil {}".to_vec());
        assert!(matches!(
            verify_contents(&changed, "@acme/lib", "1.0.0"),
            Err(RegistryError::Checksum { .. })
        ));

        read.insert("contracts/extra.veld".into(), b"x".to_vec());
        assert!(verify_contents(&read, "@acme/lib", "1.0.0").is_err());
        read.remove("contracts/extra.veld");
        read.remove("contracts/b.veld");
        assert!(verify_contents(&read, "@acme/lib", "1.0.0").is_err());
        read.remove(MANIFEST_FILE);
        assert!(verify_contents(&read, "@acme/lib", "1.0.0").is_err());
    }
}

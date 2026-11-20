//! The `.veld-manifest.json` file: which files Veld owns in an output dir.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{OutputError, io_at};
use super::safety::is_link;
use crate::fileset::{FileSet, MANIFEST_NAME, validate_rel_path};

const VERSION: u32 = 1;
const GENERATOR: &str = "veld 2.0";

#[derive(Serialize, Deserialize)]
struct Wire {
    version: u32,
    generator: String,
    files: BTreeMap<String, String>,
}

/// Relative path -> lowercase hex SHA-256 of the generated content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub files: BTreeMap<String, String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Manifest {
    pub fn from_files(files: &FileSet) -> Self {
        Self {
            files: files
                .iter()
                .map(|(p, b)| (p.to_string(), sha256_hex(b)))
                .collect(),
        }
    }

    /// Deterministic bytes: sorted keys, pretty JSON, trailing newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>, OutputError> {
        let wire = Wire {
            version: VERSION,
            generator: GENERATOR.into(),
            files: self.files.clone(),
        };
        let mut bytes =
            serde_json::to_vec_pretty(&wire).map_err(|e| OutputError::Internal(e.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Loads the manifest in `out`; `None` if there is none.
    pub fn load(out: &Path) -> Result<Option<Self>, OutputError> {
        let path = out.join(MANIFEST_NAME);
        let invalid = |reason: String| OutputError::ManifestInvalid {
            path: path.clone(),
            reason,
        };
        match fs::symlink_metadata(&path) {
            Ok(meta) if is_link(&meta) => return Err(OutputError::Symlink { path }),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_at(&path)(e)),
        }
        let bytes = fs::read(&path).map_err(io_at(&path))?;
        let wire: Wire = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
        if wire.version != VERSION {
            return Err(invalid(format!(
                "unsupported manifest version {}",
                wire.version
            )));
        }
        for key in wire.files.keys() {
            validate_rel_path(key).map_err(|e| invalid(format!("bad path `{key}`: {e}")))?;
        }
        Ok(Some(Self { files: wire.files }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn bytes_are_sorted_and_newline_terminated() {
        let mut fs = FileSet::new();
        fs.add("b.txt", "2").unwrap();
        fs.add("a.txt", "1").unwrap();
        let bytes = Manifest::from_files(&fs).to_bytes().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.ends_with("}\n"));
        assert!(text.find("a.txt").unwrap() < text.find("b.txt").unwrap());
        assert!(
            text.starts_with(
                "{\n  \"version\": 1,\n  \"generator\": \"veld 2.0\",\n  \"files\": {"
            )
        );
    }

    #[test]
    fn load_round_trip_and_rejects_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(Manifest::load(tmp.path()).unwrap(), None);
        let mut fs = FileSet::new();
        fs.add("a.txt", "1").unwrap();
        let m = Manifest::from_files(&fs);
        std::fs::write(tmp.path().join(MANIFEST_NAME), m.to_bytes().unwrap()).unwrap();
        assert_eq!(Manifest::load(tmp.path()).unwrap(), Some(m));
        std::fs::write(
            tmp.path().join(MANIFEST_NAME),
            r#"{"version":1,"generator":"x","files":{"../evil":"00"}}"#,
        )
        .unwrap();
        assert!(matches!(
            Manifest::load(tmp.path()),
            Err(OutputError::ManifestInvalid { .. })
        ));
        std::fs::write(tmp.path().join(MANIFEST_NAME), "not json").unwrap();
        assert!(Manifest::load(tmp.path()).is_err());
    }
}

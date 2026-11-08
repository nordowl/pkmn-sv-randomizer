use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("Could not read the keys file \"{}\": {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },
    #[error("The keys file has a malformed value for `{name}`. Dump your keys again.")]
    Malformed { name: String },
}

#[derive(Debug, thiserror::Error)]
#[error("the keys file does not contain `{name}`")]
pub(crate) struct MissingKey {
    pub(crate) name: String,
}

pub struct Keys {
    header_key: Option<[u8; 32]>,
    key_area_application: BTreeMap<u8, [u8; 16]>,
    titlekek: BTreeMap<u8, [u8; 16]>,
}

impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keys")
            .field("header_key_present", &self.header_key.is_some())
            .field("key_area_key_count", &self.key_area_application.len())
            .field("titlekek_count", &self.titlekek.len())
            .finish()
    }
}

impl Keys {
    pub fn load(path: &Path) -> Result<Keys, KeyError> {
        let text = fs::read_to_string(path).map_err(|source| KeyError::Unreadable {
            path: path.to_owned(),
            source,
        })?;
        parse(&text)
    }

    pub(crate) fn header_key(&self) -> Result<&[u8; 32], MissingKey> {
        self.header_key.as_ref().ok_or_else(|| MissingKey {
            name: "header_key".to_owned(),
        })
    }

    pub(crate) fn key_area_key(&self, key_generation: u8) -> Result<&[u8; 16], MissingKey> {
        let revision = revision_of(key_generation);
        self.key_area_application
            .get(&revision)
            .ok_or_else(|| MissingKey {
                name: format!("key_area_key_application_{revision:02x}"),
            })
    }

    pub(crate) fn titlekek(&self, key_generation: u8) -> Result<&[u8; 16], MissingKey> {
        let revision = revision_of(key_generation);
        self.titlekek.get(&revision).ok_or_else(|| MissingKey {
            name: format!("titlekek_{revision:02x}"),
        })
    }
}

#[cfg(test)]
impl Keys {
    pub(crate) fn scramble_title_keks(&mut self) {
        self.titlekek.values_mut().for_each(|key| key[0] ^= 0xFF);
    }

    pub(crate) fn scramble_key_area_keys(&mut self) {
        self.key_area_application
            .values_mut()
            .for_each(|key| key[0] ^= 0xFF);
    }
}

fn revision_of(key_generation: u8) -> u8 {
    key_generation.saturating_sub(1)
}

fn parse(text: &str) -> Result<Keys, KeyError> {
    let mut keys = Keys {
        header_key: None,
        key_area_application: BTreeMap::new(),
        titlekek: BTreeMap::new(),
    };
    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name == "header_key" {
            keys.header_key = Some(decode(name, value)?);
        } else if let Some(revision) = revision_suffix(name, "key_area_key_application_") {
            keys.key_area_application
                .insert(revision, decode(name, value)?);
        } else if let Some(revision) = revision_suffix(name, "titlekek_") {
            keys.titlekek.insert(revision, decode(name, value)?);
        }
    }
    Ok(keys)
}

fn revision_suffix(name: &str, prefix: &str) -> Option<u8> {
    u8::from_str_radix(name.strip_prefix(prefix)?, 16).ok()
}

fn decode<const N: usize>(name: &str, value: &str) -> Result<[u8; N], KeyError> {
    let malformed = || KeyError::Malformed {
        name: name.to_owned(),
    };
    let digits = value.as_bytes();
    if digits.len() != N * 2 {
        return Err(malformed());
    }
    let mut out = [0; N];
    for (byte, pair) in out.iter_mut().zip(digits.chunks_exact(2)) {
        let high = char::from(pair[0]).to_digit(16).ok_or_else(malformed)?;
        let low = char::from(pair[1]).to_digit(16).ok_or_else(malformed)?;
        *byte = (high << 4 | low) as u8;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
header_key = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
key_area_key_application_00 = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
key_area_key_application_10 = bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
key_area_key_application_source = not-hex-and-not-needed
titlekek_10 = cccccccccccccccccccccccccccccccc
titlek = ignored
not a key line
";

    #[test]
    fn key_generation_maps_to_revision_minus_one() {
        let keys = parse(SAMPLE).unwrap();
        assert_eq!(keys.titlekek(0x11).unwrap(), &[0xCC; 16]);
        assert_eq!(keys.key_area_key(0).unwrap(), &[0xAA; 16]);
        assert_eq!(keys.key_area_key(1).unwrap(), &[0xAA; 16]);
        assert_eq!(keys.key_area_key(0x11).unwrap(), &[0xBB; 16]);
    }

    #[test]
    fn missing_key_error_names_the_key() {
        let keys = parse(SAMPLE).unwrap();
        assert_eq!(keys.titlekek(0x02).unwrap_err().name, "titlekek_01");
        let empty = parse("").unwrap();
        assert_eq!(empty.header_key().unwrap_err().name, "header_key");
    }

    #[test]
    fn debug_and_errors_never_reveal_values() {
        let keys = parse(SAMPLE).unwrap();
        let shown = format!("{keys:?}");
        for needle in ["aaaa", "bbbb", "cccc", "170", "204", "0, 1, 2"] {
            assert!(!shown.contains(needle), "{shown}");
        }
        let error = parse("titlekek_00 = deadbeefzz").unwrap_err().to_string();
        assert!(error.contains("titlekek_00"));
        assert!(!error.contains("deadbeef"));
    }
}

use std::collections::HashMap;

use super::TextError;

const MAGIC: &[u8; 4] = b"AHTB";
// Not the standard FNV offset basis (...2325): the game's tables use ...2645, as pkNX does.
const FNV_BASIS: u64 = 0xCBF2_9CE4_8422_2645;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// An `AHTB` label table: one `(hash, name)` pair per line of the matching `.dat`.
#[derive(Clone, Debug)]
pub struct LabelTable {
    entries: Vec<Label>,
    by_hash: HashMap<u64, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub hash: u64,
    pub name: String,
}

impl LabelTable {
    pub fn label_hash(label: &str) -> u64 {
        label
            .bytes()
            .fold(FNV_BASIS, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME))
    }

    pub fn parse(tbl: &[u8]) -> Result<LabelTable, TextError> {
        let bad = |detail: &str| TextError::BadTable(detail.to_owned());
        if tbl.get(..4) != Some(MAGIC.as_slice()) {
            return Err(bad("missing AHTB magic"));
        }
        let count_bytes = tbl.get(4..8).ok_or_else(|| bad("header is truncated"))?;
        let count = i32::from_le_bytes([count_bytes[0], count_bytes[1], count_bytes[2], count_bytes[3]]);
        let count = usize::try_from(count).map_err(|_| bad("negative entry count"))?;

        let mut entries = Vec::with_capacity(count.min(tbl.len() / 11));
        let mut at = 8usize;
        for _ in 0..count {
            let head = tbl.get(at..at + 10).ok_or_else(|| bad("entry is truncated"))?;
            let mut hash = [0u8; 8];
            hash.copy_from_slice(&head[..8]);
            let len = usize::from(u16::from_le_bytes([head[8], head[9]]));
            let raw = tbl
                .get(at + 10..at + 10 + len)
                .ok_or_else(|| bad("name is truncated"))?;
            let name = match raw.split_last() {
                Some((0, name)) => name,
                _ => return Err(bad("name is not null terminated")),
            };
            let name = std::str::from_utf8(name).map_err(|_| bad("name is not UTF-8"))?;
            entries.push(Label {
                hash: u64::from_le_bytes(hash),
                name: name.to_owned(),
            });
            at += 10 + len;
        }
        if at != tbl.len() {
            return Err(bad("unexpected data after the last entry"));
        }
        Ok(LabelTable::from_entries(entries))
    }

    fn from_entries(entries: Vec<Label>) -> LabelTable {
        let mut by_hash = HashMap::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            by_hash.entry(entry.hash).or_insert(index);
        }
        LabelTable { entries, by_hash }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(self.entries.len() as i32).to_le_bytes());
        for entry in &self.entries {
            out.extend_from_slice(&entry.hash.to_le_bytes());
            out.extend_from_slice(&((entry.name.len() + 1) as u16).to_le_bytes());
            out.extend_from_slice(entry.name.as_bytes());
            out.push(0);
        }
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Label] {
        &self.entries
    }

    /// Lookup is by hash, so a label matches even if the stored name differs.
    pub fn index_of(&self, label: &str) -> Option<usize> {
        self.by_hash.get(&Self::label_hash(label)).copied()
    }

    /// Appends a label for a `.dat` that gained a line. Returns its line index.
    pub fn push(&mut self, label: &str) -> Result<usize, TextError> {
        if label.len() >= usize::from(u16::MAX) {
            return Err(TextError::BadTable("label is too long".to_owned()));
        }
        let index = self.entries.len();
        let hash = Self::label_hash(label);
        self.entries.push(Label {
            hash,
            name: label.to_owned(),
        });
        self.by_hash.entry(hash).or_insert(index);
        Ok(index)
    }
}

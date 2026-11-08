use std::io;

use super::storage::Storage;
use super::{invalid_data, le_u32, le_u64};

#[derive(Clone, Copy)]
pub(crate) enum PartitionKind {
    Hfs0,
    Pfs0,
}

impl PartitionKind {
    pub(crate) fn magic(self) -> &'static [u8; 4] {
        match self {
            PartitionKind::Hfs0 => b"HFS0",
            PartitionKind::Pfs0 => b"PFS0",
        }
    }

    fn entry_size(self) -> u64 {
        match self {
            PartitionKind::Hfs0 => 0x40,
            PartitionKind::Pfs0 => 0x18,
        }
    }
}

pub(crate) struct PartitionEntry {
    pub(crate) name: String,
    pub(crate) offset: u64,
    pub(crate) size: u64,
}

pub(crate) fn has_magic(file: &dyn Storage, offset: u64, magic: &[u8; 4]) -> io::Result<bool> {
    if offset.checked_add(4).is_none_or(|end| end > file.len()) {
        return Ok(false);
    }
    let mut found = [0; 4];
    file.read_at(offset, &mut found)?;
    Ok(&found == magic)
}

/// Entry offsets in the result are absolute within `file`.
pub(crate) fn parse_partition(
    file: &dyn Storage,
    base: u64,
    kind: PartitionKind,
) -> io::Result<Vec<PartitionEntry>> {
    let head = file.read_vec(base, 0x10)?;
    if &head[..4] != kind.magic() {
        return Err(invalid_data(format!(
            "expected a {} partition at {base:#x}",
            String::from_utf8_lossy(kind.magic())
        )));
    }
    let count = u64::from(le_u32(&head, 4)?);
    let strings_size = u64::from(le_u32(&head, 8)?);
    let entries_size = count * kind.entry_size();
    let entries = file.read_vec(base + 0x10, entries_size)?;
    let strings = file.read_vec(base + 0x10 + entries_size, strings_size)?;
    let data_base = base + 0x10 + entries_size + strings_size;

    let mut out = Vec::new();
    for raw in entries.chunks_exact(kind.entry_size() as usize) {
        let relative = le_u64(raw, 0)?;
        let size = le_u64(raw, 8)?;
        let name_offset = le_u32(raw, 16)? as usize;
        let name = entry_name(&strings, name_offset)?;
        let offset = data_base
            .checked_add(relative)
            .filter(|start| start.checked_add(size).is_some_and(|end| end <= file.len()))
            .ok_or_else(|| {
                invalid_data(format!("\"{name}\" goes past the end of the file (truncated?)"))
            })?;
        out.push(PartitionEntry { name, offset, size });
    }
    Ok(out)
}

fn entry_name(strings: &[u8], offset: usize) -> io::Result<String> {
    let bad = || invalid_data(format!("bad partition name at {offset:#x}"));
    let tail = strings.get(offset..).ok_or_else(bad)?;
    let end = tail.iter().position(|&b| b == 0).ok_or_else(bad)?;
    std::str::from_utf8(&tail[..end])
        .map(str::to_owned)
        .map_err(|_| bad())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pfs0(entry_offset: u64, entry_size: u64) -> Vec<u8> {
        let mut raw = b"PFS0".to_vec();
        raw.extend(1u32.to_le_bytes());
        raw.extend(5u32.to_le_bytes());
        raw.extend(0u32.to_le_bytes());
        raw.extend(entry_offset.to_le_bytes());
        raw.extend(entry_size.to_le_bytes());
        raw.extend(0u32.to_le_bytes());
        raw.extend(0u32.to_le_bytes());
        raw.extend(b"main\0");
        raw.extend(b"payload");
        raw
    }

    #[test]
    fn parses_names_and_absolute_offsets() {
        let file = pfs0(0, 7);
        let entries = parse_partition(&file, 0, PartitionKind::Pfs0).unwrap();
        assert_eq!(entries[0].name, "main");
        assert_eq!(entries[0].offset, 0x10 + 0x18 + 5);
        assert_eq!(file.read_vec(entries[0].offset, 7).unwrap(), b"payload");
    }

    #[test]
    fn truncated_or_wrong_files_are_errors() {
        assert!(parse_partition(&pfs0(0, 8), 0, PartitionKind::Pfs0).is_err());
        assert!(parse_partition(&pfs0(u64::MAX, 1), 0, PartitionKind::Pfs0).is_err());
        assert!(parse_partition(&pfs0(0, 7), 0, PartitionKind::Hfs0).is_err());
        assert!(!has_magic(&vec![0u8; 2], 0, b"PFS0").unwrap());
    }
}

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use super::storage::{Storage, SubStorage};
use super::{invalid_data, le_u32, le_u64};

const NO_ENTRY: u32 = u32::MAX;
const DIR_ENTRY_SIZE: usize = 0x18;
const FILE_ENTRY_SIZE: usize = 0x20;

#[derive(Debug, thiserror::Error)]
pub enum RomFsError {
    #[error("The game's RomFS has no file \"{path}\".")]
    NotFound { path: String },
    #[error("Could not read \"{path}\" from the game's RomFS: {source}")]
    Io { path: String, source: io::Error },
}

#[derive(Clone, Copy)]
struct FileExtent {
    offset: u64,
    size: u64,
}

pub(crate) struct RomFs {
    data: Arc<dyn Storage>,
    files: HashMap<String, FileExtent>,
}

impl RomFs {
    pub(crate) fn parse(data: Arc<dyn Storage>) -> io::Result<RomFs> {
        let header = data.read_vec(0, 0x50)?;
        let dir_meta = data.read_vec(le_u64(&header, 0x18)?, le_u64(&header, 0x20)?)?;
        let file_meta = data.read_vec(le_u64(&header, 0x38)?, le_u64(&header, 0x40)?)?;
        let files = walk(&dir_meta, &file_meta, le_u64(&header, 0x48)?)?;
        Ok(RomFs { data, files })
    }

    pub(crate) fn read(&self, path: &str) -> Result<Vec<u8>, RomFsError> {
        let extent = self.extent(path)?;
        self.data
            .read_vec(extent.offset, extent.size)
            .map_err(|source| RomFsError::Io {
                path: path.to_owned(),
                source,
            })
    }

    pub(crate) fn file_storage(&self, path: &str) -> Result<SubStorage, RomFsError> {
        let extent = self.extent(path)?;
        SubStorage::new(self.data.clone(), extent.offset, extent.size).map_err(|source| {
            RomFsError::Io {
                path: path.to_owned(),
                source,
            }
        })
    }

    fn extent(&self, path: &str) -> Result<FileExtent, RomFsError> {
        self.files
            .get(path)
            .copied()
            .ok_or_else(|| RomFsError::NotFound {
                path: path.to_owned(),
            })
    }
}

fn walk(
    dir_meta: &[u8],
    file_meta: &[u8],
    data_offset: u64,
) -> io::Result<HashMap<String, FileExtent>> {
    let cycle = || invalid_data("RomFS metadata links entries in a cycle");
    // Every entry occupies at least its fixed header, so a longer walk can only be a cycle.
    let mut dirs_left = dir_meta.len() / DIR_ENTRY_SIZE;
    let mut files_left = file_meta.len() / FILE_ENTRY_SIZE;

    let mut files = HashMap::new();
    let mut pending = vec![(0u32, String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        dirs_left = dirs_left.checked_sub(1).ok_or_else(cycle)?;
        let at = dir as usize;
        let sibling = le_u32(dir_meta, at + 0x4)?;
        let child = le_u32(dir_meta, at + 0x8)?;
        let mut file = le_u32(dir_meta, at + 0xC)?;
        let name = entry_name(dir_meta, at + DIR_ENTRY_SIZE, le_u32(dir_meta, at + 0x14)?)?;
        let here = if dir == 0 {
            String::new()
        } else {
            format!("{prefix}{name}/")
        };

        while file != NO_ENTRY {
            files_left = files_left.checked_sub(1).ok_or_else(cycle)?;
            let at = file as usize;
            let offset = le_u64(file_meta, at + 0x8)?;
            let size = le_u64(file_meta, at + 0x10)?;
            let name = entry_name(file_meta, at + FILE_ENTRY_SIZE, le_u32(file_meta, at + 0x1C)?)?;
            let offset = data_offset
                .checked_add(offset)
                .ok_or_else(|| invalid_data("RomFS file offset overflows"))?;
            files.insert(format!("{here}{name}"), FileExtent { offset, size });
            file = le_u32(file_meta, at + 0x4)?;
        }

        if child != NO_ENTRY {
            pending.push((child, here));
        }
        if sibling != NO_ENTRY {
            pending.push((sibling, prefix));
        }
    }
    Ok(files)
}

fn entry_name(meta: &[u8], at: usize, len: u32) -> io::Result<&str> {
    at.checked_add(len as usize)
        .and_then(|end| meta.get(at..end))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| invalid_data(format!("bad RomFS entry name at {at:#x}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_entry(sibling: u32, child: u32, file: u32, name: &str) -> Vec<u8> {
        let mut entry = Vec::new();
        for field in [0, sibling, child, file, 0, name.len() as u32] {
            entry.extend(field.to_le_bytes());
        }
        entry.extend(name.as_bytes());
        entry
    }

    fn file_entry(sibling: u32, offset: u64, size: u64, name: &str) -> Vec<u8> {
        let mut entry = Vec::new();
        entry.extend(0u32.to_le_bytes());
        entry.extend(sibling.to_le_bytes());
        entry.extend(offset.to_le_bytes());
        entry.extend(size.to_le_bytes());
        entry.extend(0u32.to_le_bytes());
        entry.extend((name.len() as u32).to_le_bytes());
        entry.extend(name.as_bytes());
        entry
    }

    #[test]
    fn walks_nested_directories_and_sibling_chains() {
        let top = file_entry(NO_ENTRY, 0x10, 4, "top.txt");
        let first_arc_file = top.len() as u32;
        let packs = file_entry(first_arc_file + 0x2A, 0x20, 8, "data.trpfs");
        let index = file_entry(NO_ENTRY, 0x40, 16, "data.trpfd");
        assert_eq!(packs.len(), 0x2A);
        let meta = [top, packs, index].concat();

        let root = dir_entry(NO_ENTRY, DIR_ENTRY_SIZE as u32, 0, "");
        let arc_len = DIR_ENTRY_SIZE as u32 + 3;
        let arc = dir_entry(DIR_ENTRY_SIZE as u32 + arc_len, NO_ENTRY, first_arc_file, "arc");
        let other = dir_entry(NO_ENTRY, NO_ENTRY, NO_ENTRY, "other");
        let dirs = [root, arc, other].concat();

        let files = walk(&dirs, &meta, 0x1000).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files["top.txt"].offset, 0x1010);
        assert_eq!(files["arc/data.trpfs"].offset, 0x1020);
        assert_eq!(files["arc/data.trpfd"].size, 16);
    }

    #[test]
    fn cycles_in_metadata_are_errors_not_hangs() {
        let looping_dir = dir_entry(0, NO_ENTRY, NO_ENTRY, "");
        assert!(walk(&looping_dir, &[], 0).is_err());

        let root = dir_entry(NO_ENTRY, NO_ENTRY, 0, "");
        let looping_file = file_entry(0, 0, 0, "a");
        assert!(walk(&root, &looping_file, 0).is_err());
    }

    #[test]
    fn out_of_bounds_metadata_is_an_error() {
        let root = dir_entry(NO_ENTRY, NO_ENTRY, 0x1000, "");
        assert!(walk(&root, &[], 0).is_err());
        assert!(walk(&root[..4], &[], 0).is_err());
    }
}

use std::io;
use std::sync::Arc;

use super::crypto::{CtrStorage, NcaCipher};
use super::nca::{BucketTable, NcaSection};
use super::storage::Storage;
use super::{invalid_data, le_u32, le_u64};

const NODE_SIZE: usize = 0x4000;
const NODE_HEADER_SIZE: usize = 0x10;
const INDIRECT_ENTRY_SIZE: usize = 0x14;
const AES_EX_ENTRY_SIZE: usize = 0x10;

#[derive(Clone, Copy)]
enum Source {
    Base,
    Patch,
}

struct IndirectEntry {
    virtual_offset: u64,
    physical_offset: u64,
    source: Source,
}

struct AesExEntry {
    offset: u64,
    generation: u32,
}

/// The RomFS section an update presents: extents of the base section mixed with its own patch data.
pub(crate) struct PatchedStorage {
    base: Arc<dyn Storage>,
    patch: NcaCipher,
    section_start: u64,
    counter_high: [u8; 8],
    indirect: Vec<IndirectEntry>,
    virtual_end: u64,
    aes_ex: Vec<AesExEntry>,
    aes_ex_end: u64,
}

impl PatchedStorage {
    pub(crate) fn new(
        base: Arc<dyn Storage>,
        patch: NcaCipher,
        section: &NcaSection,
        indirect_table: BucketTable,
        aes_ex_table: BucketTable,
    ) -> io::Result<PatchedStorage> {
        let plain = CtrStorage::new(
            patch.clone(),
            section.start,
            section.len,
            section.counter_high,
        );
        let (indirect, virtual_end) =
            read_bucket_tree(&plain, indirect_table, INDIRECT_ENTRY_SIZE, parse_indirect)?;
        let (aes_ex, _) = read_bucket_tree(&plain, aes_ex_table, AES_EX_ENTRY_SIZE, parse_aes_ex)?;

        if indirect.first().map(|entry| entry.virtual_offset) != Some(0)
            || !indirect
                .windows(2)
                .all(|pair| pair[0].virtual_offset < pair[1].virtual_offset)
            || indirect
                .last()
                .is_none_or(|entry| entry.virtual_offset >= virtual_end)
        {
            return Err(invalid_data("BKTR indirect table is not in order"));
        }
        if !aes_ex.windows(2).all(|pair| pair[0].offset < pair[1].offset) {
            return Err(invalid_data("BKTR AES-CTR-EX table is not in order"));
        }

        Ok(PatchedStorage {
            base,
            patch,
            section_start: section.start,
            counter_high: section.counter_high,
            indirect,
            virtual_end,
            aes_ex,
            aes_ex_end: aes_ex_table.offset,
        })
    }

    fn read_patch(&self, physical: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut done = 0;
        while done < buf.len() {
            let pos = physical + done as u64;
            let next_index = self.aes_ex.partition_point(|entry| entry.offset <= pos);
            let entry = next_index
                .checked_sub(1)
                .map(|index| &self.aes_ex[index])
                .ok_or_else(|| invalid_data(format!("patch offset {pos:#x} has no AES entry")))?;
            let next = self
                .aes_ex
                .get(next_index)
                .map_or(self.aes_ex_end, |entry| entry.offset);
            let step = step_within(pos, next, buf.len() - done)?;

            let mut counter_high = self.counter_high;
            counter_high[4..].copy_from_slice(&entry.generation.to_be_bytes());
            self.patch.read(
                self.section_start + pos,
                counter_high,
                &mut buf[done..done + step],
            )?;
            done += step;
        }
        Ok(())
    }
}

impl Storage for PatchedStorage {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut done = 0;
        while done < buf.len() {
            let pos = offset + done as u64;
            let next_index = self
                .indirect
                .partition_point(|entry| entry.virtual_offset <= pos);
            let entry = &self.indirect[next_index - 1];
            let next = self
                .indirect
                .get(next_index)
                .map_or(self.virtual_end, |entry| entry.virtual_offset);
            let step = step_within(pos, next, buf.len() - done)?;

            let physical = entry
                .physical_offset
                .checked_add(pos - entry.virtual_offset)
                .ok_or_else(|| invalid_data("BKTR physical offset overflows"))?;
            let chunk = &mut buf[done..done + step];
            match entry.source {
                Source::Base => self.base.read_at(physical, chunk)?,
                Source::Patch => self.read_patch(physical, chunk)?,
            }
            done += step;
        }
        Ok(())
    }

    fn len(&self) -> u64 {
        self.virtual_end
    }
}

/// A wrong key produces tables that send readers past their end, so a non-positive step is an
/// error rather than a loop that never finishes.
fn step_within(pos: u64, next: u64, remaining: usize) -> io::Result<usize> {
    match next.checked_sub(pos) {
        Some(available) if available > 0 => Ok(available.min(remaining as u64) as usize),
        _ => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("BKTR read at {pos:#x} is past the end of its extent ({next:#x})"),
        )),
    }
}

fn read_bucket_tree<T>(
    plain: &dyn Storage,
    table: BucketTable,
    entry_size: usize,
    parse_entry: impl Fn(&[u8]) -> io::Result<T>,
) -> io::Result<(Vec<T>, u64)> {
    let count = table.count as usize;
    let per_node = (NODE_SIZE - NODE_HEADER_SIZE) / entry_size;
    if count.div_ceil(per_node) > (NODE_SIZE - NODE_HEADER_SIZE) / 8 {
        return Err(invalid_data("BKTR bucket tree needs an unsupported L2 level"));
    }

    let raw = plain.read_vec(table.offset, table.size)?;
    let node_count = le_u32(&raw, 4)? as usize;
    let end = le_u64(&raw, 8)?;

    let mut entries = Vec::new();
    for node_index in 0..node_count {
        let node = (node_index + 1) * NODE_SIZE;
        let in_node = le_u32(&raw, node + 4)? as usize;
        if in_node > per_node {
            return Err(invalid_data(format!(
                "BKTR node {node_index} claims {in_node} entries"
            )));
        }
        for slot in 0..in_node {
            let at = node + NODE_HEADER_SIZE + slot * entry_size;
            let bytes = raw
                .get(at..at + entry_size)
                .ok_or_else(|| invalid_data("BKTR entry is out of bounds"))?;
            entries.push(parse_entry(bytes)?);
        }
    }
    if entries.len() != count {
        return Err(invalid_data(format!(
            "BKTR table has {} entries, expected {count}",
            entries.len()
        )));
    }
    Ok((entries, end))
}

fn parse_indirect(bytes: &[u8]) -> io::Result<IndirectEntry> {
    let source = match le_u32(bytes, 16)? {
        0 => Source::Base,
        1 => Source::Patch,
        other => return Err(invalid_data(format!("BKTR storage index {other}"))),
    };
    Ok(IndirectEntry {
        virtual_offset: le_u64(bytes, 0)?,
        physical_offset: le_u64(bytes, 8)?,
        source,
    })
}

fn parse_aes_ex(bytes: &[u8]) -> io::Result<AesExEntry> {
    Ok(AesExEntry {
        offset: le_u64(bytes, 0)?,
        generation: le_u32(bytes, 12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::super::crypto::encrypt_in_place;
    use super::super::nca::{SectionEncryption, SectionFormat};
    use super::*;

    const KEY: [u8; 16] = [0x11; 16];
    const COUNTER_HIGH: [u8; 8] = [1, 2, 3, 4, 0, 0, 0, 9];
    const SECTION_START: usize = 0x1000;
    const INDIRECT_OFFSET: usize = 0x400;
    const AES_EX_OFFSET: usize = INDIRECT_OFFSET + 2 * NODE_SIZE;
    const VIRTUAL_END: u64 = 0x300;

    struct Fixture {
        file: Vec<u8>,
        indirect: BucketTable,
        aes_ex: BucketTable,
    }

    fn put_tree(file: &mut [u8], at: usize, entries: &[Vec<u8>], end: u64) {
        file[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
        file[at + 8..at + 16].copy_from_slice(&end.to_le_bytes());
        let node = at + NODE_SIZE;
        file[node + 4..node + 8].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        let mut pos = node + NODE_HEADER_SIZE;
        for entry in entries {
            file[pos..pos + entry.len()].copy_from_slice(entry);
            pos += entry.len();
        }
    }

    fn indirect(virtual_offset: u64, physical_offset: u64, storage: u32) -> Vec<u8> {
        let mut entry = virtual_offset.to_le_bytes().to_vec();
        entry.extend(physical_offset.to_le_bytes());
        entry.extend(storage.to_le_bytes());
        entry
    }

    fn aes_ex(offset: u64, generation: u32) -> Vec<u8> {
        let mut entry = offset.to_le_bytes().to_vec();
        entry.extend([0; 4]);
        entry.extend(generation.to_le_bytes());
        entry
    }

    fn fixture() -> Fixture {
        let table_size = 2 * NODE_SIZE;
        let mut file = vec![0; SECTION_START + AES_EX_OFFSET + table_size];
        for (i, byte) in file[SECTION_START..SECTION_START + 0x100].iter_mut().enumerate() {
            *byte = 0x80 | (i % 0x80) as u8;
        }
        let tables = SECTION_START + INDIRECT_OFFSET;
        put_tree(
            &mut file,
            tables,
            &[indirect(0, 0, 0), indirect(0x100, 0, 1), indirect(0x200, 0x200, 0)],
            VIRTUAL_END,
        );
        put_tree(
            &mut file,
            SECTION_START + AES_EX_OFFSET,
            &[aes_ex(0, 7), aes_ex(0x80, 9)],
            0,
        );
        encrypt_in_place(&mut file, KEY, tables, table_size, COUNTER_HIGH);
        encrypt_in_place(
            &mut file,
            KEY,
            SECTION_START + AES_EX_OFFSET,
            table_size,
            COUNTER_HIGH,
        );
        encrypt_in_place(&mut file, KEY, SECTION_START, 0x80, [1, 2, 3, 4, 0, 0, 0, 7]);
        encrypt_in_place(
            &mut file,
            KEY,
            SECTION_START + 0x80,
            0x80,
            [1, 2, 3, 4, 0, 0, 0, 9],
        );
        Fixture {
            file,
            indirect: BucketTable {
                offset: INDIRECT_OFFSET as u64,
                size: table_size as u64,
                count: 3,
            },
            aes_ex: BucketTable {
                offset: AES_EX_OFFSET as u64,
                size: table_size as u64,
                count: 2,
            },
        }
    }

    fn base_bytes() -> Vec<u8> {
        (0..0x300u32).map(|i| (i % 0x7F) as u8).collect()
    }

    fn open(fixture: Fixture, key: [u8; 16]) -> io::Result<PatchedStorage> {
        let section = NcaSection {
            start: SECTION_START as u64,
            len: (fixture.file.len() - SECTION_START) as u64,
            counter_high: COUNTER_HIGH,
            format: SectionFormat::Other,
            encryption: SectionEncryption::Ctr,
        };
        PatchedStorage::new(
            Arc::new(base_bytes()),
            NcaCipher::new(Arc::new(fixture.file), 0, key),
            &section,
            fixture.indirect,
            fixture.aes_ex,
        )
    }

    #[test]
    fn reads_across_base_and_patch_extents_and_generations() {
        let patched = open(fixture(), KEY).unwrap();
        let base = base_bytes();
        let got = patched.read_vec(0xF0, 0x120).unwrap();
        assert_eq!(got[..0x10], base[0xF0..0x100]);
        let patch: Vec<u8> = (0..0x100).map(|i| 0x80 | (i % 0x80) as u8).collect();
        assert_eq!(got[0x10..0x110], patch[..]);
        assert_eq!(got[0x110..], base[0x200..0x210]);
    }

    #[test]
    fn reading_past_the_virtual_end_is_an_error_not_a_hang() {
        let patched = open(fixture(), KEY).unwrap();
        let mut buf = [0; 1];
        assert!(patched.read_at(VIRTUAL_END, &mut buf).is_err());
        let mut buf = [0; 0x20];
        assert!(patched.read_at(VIRTUAL_END - 0x10, &mut buf).is_err());
        assert!(patched.read_at(u64::MAX, &mut buf).is_err());
    }

    #[test]
    fn wrong_key_is_a_clean_error() {
        assert!(open(fixture(), [0x22; 16]).is_err());
    }

    #[test]
    fn step_guard_rejects_non_positive_steps() {
        assert_eq!(step_within(5, 9, 100).unwrap(), 4);
        assert_eq!(step_within(5, 9, 2).unwrap(), 2);
        assert!(step_within(9, 9, 1).is_err());
        assert!(step_within(10, 9, 1).is_err());
    }

    #[test]
    fn trees_that_would_need_l2_are_refused() {
        let mut fixture = fixture();
        let per_node = (NODE_SIZE - NODE_HEADER_SIZE) / INDIRECT_ENTRY_SIZE;
        fixture.indirect.count = (per_node * ((NODE_SIZE - NODE_HEADER_SIZE) / 8 + 1)) as u32;
        let error = open(fixture, KEY).err().unwrap();
        assert!(error.to_string().contains("L2"), "{error}");
    }

    #[test]
    fn out_of_order_tables_are_refused() {
        let mut fixture = fixture();
        let tables = SECTION_START + INDIRECT_OFFSET;
        encrypt_in_place(&mut fixture.file, KEY, tables, 2 * NODE_SIZE, COUNTER_HIGH);
        put_tree(
            &mut fixture.file,
            tables,
            &[indirect(0, 0, 0), indirect(0x200, 0, 1), indirect(0x100, 0, 0)],
            VIRTUAL_END,
        );
        encrypt_in_place(&mut fixture.file, KEY, tables, 2 * NODE_SIZE, COUNTER_HIGH);
        assert!(open(fixture, KEY).is_err());
    }
}

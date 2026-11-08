use std::io;

use super::crypto::{decrypt_nca_header, ecb_decrypt_block};
use super::keys::Keys;
use super::storage::Storage;
use super::{array_at, invalid_data, le_u32, le_u64, ContainerError};

const HEADER_SIZE: usize = 0xC00;
const MEDIA_UNIT: u64 = 0x200;
const PROGRAM_CONTENT_TYPE: u8 = 0;

pub(crate) struct NcaHeader {
    pub(crate) title_id: u64,
    pub(crate) rights_id: [u8; 16],
    pub(crate) sections: Vec<NcaSection>,
    content_type: u8,
    key_generation: u8,
    key_area_index: u8,
    encrypted_ctr_key: [u8; 16],
}

pub(crate) struct NcaSection {
    pub(crate) start: u64,
    pub(crate) len: u64,
    pub(crate) counter_high: [u8; 8],
    pub(crate) format: SectionFormat,
    pub(crate) encryption: SectionEncryption,
}

#[derive(Clone, Copy)]
pub(crate) enum SectionFormat {
    RomFs { data_offset: u64, data_size: u64 },
    Pfs0 { data_offset: u64 },
    Other,
}

#[derive(Clone, Copy)]
pub(crate) enum SectionEncryption {
    Ctr,
    CtrEx {
        indirect: BucketTable,
        aes_ex: BucketTable,
    },
    Other,
}

#[derive(Clone, Copy)]
pub(crate) struct BucketTable {
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) count: u32,
}

impl NcaHeader {
    pub(crate) fn read(
        file: &dyn Storage,
        offset: u64,
        keys: &Keys,
    ) -> Result<NcaHeader, ContainerError> {
        let key = keys.header_key()?;
        let mut raw = file.read_vec(offset, HEADER_SIZE as u64)?;
        decrypt_nca_header(key, &mut raw);
        Ok(parse_header(&raw)?)
    }

    pub(crate) fn is_program(&self) -> bool {
        self.content_type == PROGRAM_CONTENT_TYPE
    }

    pub(crate) fn has_rights_id(&self) -> bool {
        self.rights_id != [0; 16]
    }

    pub(crate) fn romfs_section(&self) -> Option<&NcaSection> {
        self.sections
            .iter()
            .find(|section| matches!(section.format, SectionFormat::RomFs { .. }))
    }

    /// The key-area key that decrypts AES-CTR sections of an NCA with no rights ID.
    pub(crate) fn key_area_ctr_key(&self, keys: &Keys) -> Result<[u8; 16], ContainerError> {
        if self.key_area_index != 0 {
            return Err(invalid_data(format!(
                "unexpected key area index {}",
                self.key_area_index
            ))
            .into());
        }
        let key = keys.key_area_key(self.key_generation)?;
        Ok(ecb_decrypt_block(key, &self.encrypted_ctr_key))
    }
}

fn parse_header(raw: &[u8]) -> io::Result<NcaHeader> {
    if array_at(raw, 0x200)? != *b"NCA3" {
        return Err(invalid_data(
            "the NCA header did not decrypt to a valid NCA3 header",
        ));
    }
    let [content_type] = array_at(raw, 0x205)?;
    let [key_generation_old] = array_at(raw, 0x206)?;
    let [key_area_index] = array_at(raw, 0x207)?;
    let [key_generation_new] = array_at(raw, 0x220)?;

    let mut sections = Vec::new();
    for index in 0..4 {
        let table_entry = 0x240 + index * 0x10;
        let start = u64::from(le_u32(raw, table_entry)?) * MEDIA_UNIT;
        let end = u64::from(le_u32(raw, table_entry + 4)?) * MEDIA_UNIT;
        if start == 0 && end == 0 {
            continue;
        }
        let fs_header = raw
            .get(0x400 + index * 0x200..0x600 + index * 0x200)
            .ok_or_else(|| invalid_data("NCA header is truncated"))?;
        sections.push(parse_section(fs_header, start, end)?);
    }

    Ok(NcaHeader {
        title_id: le_u64(raw, 0x210)?,
        rights_id: array_at(raw, 0x230)?,
        sections,
        content_type,
        key_generation: key_generation_old.max(key_generation_new),
        key_area_index,
        encrypted_ctr_key: array_at(raw, 0x320)?,
    })
}

fn parse_section(fs: &[u8], start: u64, end: u64) -> io::Result<NcaSection> {
    let len = end
        .checked_sub(start)
        .ok_or_else(|| invalid_data("NCA section ends before it starts"))?;
    let [_, _, fs_type, _, encryption] = array_at(fs, 0)?;
    let hash_data = fs
        .get(0x8..0x100)
        .ok_or_else(|| invalid_data("NCA section header is truncated"))?;
    let format = match fs_type {
        0 => parse_ivfc(hash_data)?,
        1 => parse_hierarchical_sha256(hash_data)?,
        _ => SectionFormat::Other,
    };
    let encryption = match encryption {
        3 => SectionEncryption::Ctr,
        4 => SectionEncryption::CtrEx {
            indirect: parse_bucket_table(fs, 0x100)?,
            aes_ex: parse_bucket_table(fs, 0x120)?,
        },
        _ => SectionEncryption::Other,
    };
    let mut counter_high: [u8; 8] = array_at(fs, 0x140)?;
    counter_high.reverse();
    Ok(NcaSection {
        start,
        len,
        counter_high,
        format,
        encryption,
    })
}

fn parse_ivfc(hash_data: &[u8]) -> io::Result<SectionFormat> {
    if array_at(hash_data, 0)? != *b"IVFC" {
        return Err(invalid_data("RomFS section has no IVFC hash header"));
    }
    let data_level = 0x10 + 5 * 0x18;
    Ok(SectionFormat::RomFs {
        data_offset: le_u64(hash_data, data_level)?,
        data_size: le_u64(hash_data, data_level + 8)?,
    })
}

fn parse_hierarchical_sha256(hash_data: &[u8]) -> io::Result<SectionFormat> {
    let last_layer = le_u32(hash_data, 0x24)?
        .checked_sub(1)
        .ok_or_else(|| invalid_data("PFS0 section has no hash layers"))?;
    let data_offset = le_u64(hash_data, 0x28 + last_layer as usize * 0x10)?;
    Ok(SectionFormat::Pfs0 { data_offset })
}

fn parse_bucket_table(fs: &[u8], at: usize) -> io::Result<BucketTable> {
    if array_at(fs, at + 0x10)? != *b"BKTR" {
        return Err(invalid_data("patch section has no BKTR table header"));
    }
    Ok(BucketTable {
        offset: le_u64(fs, at)?,
        size: le_u64(fs, at + 8)?,
        count: le_u32(fs, at + 0x18)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_with_sections() -> Vec<u8> {
        let mut raw = vec![0; HEADER_SIZE];
        raw[0x200..0x204].copy_from_slice(b"NCA3");
        raw[0x206] = 0x03;
        raw[0x220] = 0x11;
        raw[0x210..0x218].copy_from_slice(&0x0100_A3D0_08C5_C800u64.to_le_bytes());
        raw[0x240..0x244].copy_from_slice(&6u32.to_le_bytes());
        raw[0x244..0x248].copy_from_slice(&10u32.to_le_bytes());
        let fs = 0x400;
        raw[fs + 2] = 1;
        raw[fs + 4] = 3;
        raw[fs + 8 + 0x24] = 2;
        raw[fs + 8 + 0x38..fs + 8 + 0x40].copy_from_slice(&0x4000u64.to_le_bytes());
        raw[fs + 0x140..fs + 0x148].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        raw
    }

    #[test]
    fn parses_identity_and_section_layout() {
        let header = parse_header(&header_with_sections()).unwrap();
        assert_eq!(header.title_id, 0x0100_A3D0_08C5_C800);
        assert_eq!(header.key_generation, 0x11);
        assert!(header.is_program());
        assert!(!header.has_rights_id());
        let section = &header.sections[0];
        assert_eq!((section.start, section.len), (0xC00, 0x800));
        assert_eq!(section.counter_high, [8, 7, 6, 5, 4, 3, 2, 1]);
        assert!(matches!(section.encryption, SectionEncryption::Ctr));
        assert!(matches!(
            section.format,
            SectionFormat::Pfs0 { data_offset: 0x4000 }
        ));
    }

    #[test]
    fn wrong_header_key_and_bad_layouts_are_errors() {
        assert!(parse_header(&vec![0; HEADER_SIZE]).is_err());

        let mut backwards = header_with_sections();
        backwards[0x244..0x248].copy_from_slice(&5u32.to_le_bytes());
        assert!(parse_header(&backwards).is_err());

        let mut huge_layers = header_with_sections();
        huge_layers[0x400 + 8 + 0x24..0x400 + 8 + 0x28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_header(&huge_layers).is_err());
    }
}

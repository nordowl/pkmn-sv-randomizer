use std::io;
use std::sync::Arc;

use aes::Aes128;
use cipher::generic_array::GenericArray;
use cipher::{BlockDecrypt, KeyInit, KeyIvInit, StreamCipher, StreamCipherSeek};
use xts_mode::Xts128;

use super::storage::{check_range, Storage};

type Aes128Ctr = ctr::Ctr128BE<Aes128>;

const NCA_SECTOR_SIZE: usize = 0x200;

pub(crate) fn decrypt_nca_header(header_key: &[u8; 32], area: &mut [u8]) {
    let (data_key, tweak_key) = header_key.split_at(16);
    let xts = Xts128::new(
        Aes128::new(GenericArray::from_slice(data_key)),
        Aes128::new(GenericArray::from_slice(tweak_key)),
    );
    xts.decrypt_area(area, NCA_SECTOR_SIZE, 0, u128::to_be_bytes);
}

pub(crate) fn ecb_decrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
    let mut out = GenericArray::from(*block);
    Aes128::new(GenericArray::from_slice(key)).decrypt_block(&mut out);
    out.into()
}

#[derive(Clone)]
pub(crate) struct NcaCipher {
    file: Arc<dyn Storage>,
    nca_offset: u64,
    key: [u8; 16],
}

impl NcaCipher {
    pub(crate) fn new(file: Arc<dyn Storage>, nca_offset: u64, key: [u8; 16]) -> NcaCipher {
        NcaCipher {
            file,
            nca_offset,
            key,
        }
    }

    /// `offset` is relative to the start of the NCA, because the counter is derived from it.
    pub(crate) fn read(
        &self,
        offset: u64,
        counter_high: [u8; 8],
        buf: &mut [u8],
    ) -> io::Result<()> {
        self.file.read_at(self.nca_offset + offset, buf)?;
        let mut iv = [0; 16];
        iv[..8].copy_from_slice(&counter_high);
        iv[8..].copy_from_slice(&(offset >> 4).to_be_bytes());
        let mut cipher = Aes128Ctr::new(
            GenericArray::from_slice(&self.key),
            GenericArray::from_slice(&iv),
        );
        cipher.seek(offset & 0xF);
        cipher.apply_keystream(buf);
        Ok(())
    }
}

pub(crate) struct CtrStorage {
    cipher: NcaCipher,
    start: u64,
    len: u64,
    counter_high: [u8; 8],
}

impl CtrStorage {
    pub(crate) fn new(cipher: NcaCipher, start: u64, len: u64, counter_high: [u8; 8]) -> CtrStorage {
        CtrStorage {
            cipher,
            start,
            len,
            counter_high,
        }
    }
}

impl Storage for CtrStorage {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        check_range(offset, buf.len() as u64, self.len)?;
        self.cipher
            .read(self.start + offset, self.counter_high, buf)
    }

    fn len(&self) -> u64 {
        self.len
    }
}

#[cfg(test)]
pub(crate) fn encrypt_in_place(
    file: &mut [u8],
    key: [u8; 16],
    offset: usize,
    size: usize,
    counter_high: [u8; 8],
) {
    let plain: Arc<dyn Storage> = Arc::new(file.to_vec());
    let mut out = vec![0; size];
    NcaCipher::new(plain, 0, key)
        .read(offset as u64, counter_high, &mut out)
        .unwrap();
    file[offset..offset + size].copy_from_slice(&out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unaligned_reads_match_the_aligned_keystream() {
        let key = [7; 16];
        let counter = [1, 2, 3, 4, 5, 6, 7, 8];
        let plain: Vec<u8> = (0..=255).collect();
        let mut encrypted = plain.clone();
        encrypt_in_place(&mut encrypted, key, 0, 256, counter);
        assert_ne!(encrypted, plain);

        let storage = CtrStorage::new(
            NcaCipher::new(Arc::new(encrypted), 0, key),
            0x10,
            0xF0,
            counter,
        );
        assert_eq!(storage.read_vec(0, 0xF0).unwrap(), plain[0x10..]);
        assert_eq!(storage.read_vec(5, 0x31).unwrap(), plain[0x15..0x46]);
        assert!(storage.read_vec(0xE0, 0x11).is_err());
    }
}

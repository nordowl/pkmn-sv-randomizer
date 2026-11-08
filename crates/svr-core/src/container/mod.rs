mod bktr;
mod crypto;
mod keys;
mod nca;
mod partition;
mod romfs;
mod storage;
mod ticket;

use std::io;

pub use keys::{KeyError, Keys};
pub use romfs::RomFsError;
pub use storage::{FileStorage, Storage, SubStorage};

pub(crate) use bktr::PatchedStorage;
pub(crate) use crypto::{CtrStorage, NcaCipher};
pub(crate) use keys::MissingKey;
pub(crate) use nca::{NcaHeader, SectionEncryption, SectionFormat};
pub(crate) use partition::{has_magic, parse_partition, PartitionEntry, PartitionKind};
pub(crate) use romfs::RomFs;
pub(crate) use ticket::read_title_key;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ContainerError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MissingKey(#[from] MissingKey),
}

pub(crate) fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(crate) fn array_at<const N: usize>(data: &[u8], at: usize) -> io::Result<[u8; N]> {
    at.checked_add(N)
        .and_then(|end| data.get(at..end))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| invalid_data(format!("structure field at {at:#x} is out of bounds")))
}

pub(crate) fn le_u32(data: &[u8], at: usize) -> io::Result<u32> {
    array_at(data, at).map(u32::from_le_bytes)
}

pub(crate) fn le_u64(data: &[u8], at: usize) -> io::Result<u64> {
    array_at(data, at).map(u64::from_le_bytes)
}

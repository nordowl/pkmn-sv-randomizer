use super::crypto::ecb_decrypt_block;
use super::keys::Keys;
use super::partition::PartitionEntry;
use super::storage::Storage;
use super::{array_at, invalid_data, le_u32, ContainerError};

const TICKET_SIZE: u64 = 0x2C0;
const RSA2048_SHA256_SIGNATURE: u32 = 0x10004;

pub(crate) fn read_title_key(
    file: &dyn Storage,
    entry: &PartitionEntry,
    rights_id: &[u8; 16],
    keys: &Keys,
) -> Result<[u8; 16], ContainerError> {
    if entry.size < TICKET_SIZE {
        return Err(invalid_data("the ticket is too small").into());
    }
    let ticket = file.read_vec(entry.offset, TICKET_SIZE)?;
    let signature = le_u32(&ticket, 0)?;
    if signature != RSA2048_SHA256_SIGNATURE {
        return Err(invalid_data(format!("unexpected ticket signature type {signature:#x}")).into());
    }
    let [title_key_type] = array_at(&ticket, 0x281)?;
    if title_key_type != 0 {
        return Err(invalid_data("the ticket is personalized to one console").into());
    }
    let ticket_rights: [u8; 16] = array_at(&ticket, 0x2A0)?;
    if &ticket_rights != rights_id {
        return Err(invalid_data("the ticket does not belong to the program content").into());
    }
    let [key_generation] = array_at(&ticket, 0x285)?;
    let kek = keys.titlekek(key_generation)?;
    Ok(ecb_decrypt_block(kek, &array_at(&ticket, 0x180)?))
}

//! The Poké Ball items lying around the overworld. Each ball is an object in a `world_item` scene
//! with a property sheet `itemInfo { num, itemNo }`. A property is a table `{ key, type, value }`
//! whose value is a table holding a 64-bit integer, so both numbers change in place and every
//! other byte stays as the game wrote it.

use super::DataError;

/// One ball: where its item and stack size live in the scene file, and what they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ball {
    pub item_at: usize,
    pub item: u16,
    pub num_at: Option<usize>,
    pub num: i64,
}

/// The key string as FlatBuffers stores it: length, bytes, terminating zero.
fn key(name: &str) -> Vec<u8> {
    let mut out = (name.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out
}

/// How far before a key string its property table can start. Observed: under 64 bytes.
const KEY_REACH: usize = 512;
/// How far before an `itemNo` key the same sheet's `num` key can be. Observed: 64 bytes.
const NUM_REACH: usize = 256;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn i64_at(d: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

/// The position of field `index` in the table at `table`, if the table has it.
fn field(d: &[u8], table: usize, index: usize) -> Option<usize> {
    let soffset = i32::from_le_bytes(d.get(table..table + 4)?.try_into().ok()?);
    let vtable = usize::try_from(table as i64 - i64::from(soffset)).ok()?;
    let vsize = usize::from(u16_at(d, vtable)?);
    let slot = 4 + 2 * index;
    if slot + 2 > vsize {
        return None;
    }
    let offset = usize::from(u16_at(d, vtable + slot)?);
    (offset != 0).then_some(table + offset)
}

/// The integer value of the property whose key string starts at `string`: finds the property table
/// whose first field points at the string, follows its value field, and checks the value is an
/// integer (type 4) stored in the value table's first field.
/// Bytes that merely look like a pointer to the string are skipped: only a reference that passes
/// every check counts.
fn int_property(d: &[u8], string: usize) -> Option<usize> {
    let lowest = string.saturating_sub(KEY_REACH);
    (lowest..string)
        .rev()
        .filter(|&q| u32_at(d, q).is_some_and(|u| q + u as usize == string))
        .find_map(|reference| int_value(d, reference))
}

fn int_value(d: &[u8], reference: usize) -> Option<usize> {
    let table = (reference.saturating_sub(64)..=reference)
        .rev()
        .find(|&t| field(d, t, 0) == Some(reference))?;
    let value_ref = field(d, table, 2)?;
    let value = value_ref + u32_at(d, value_ref)? as usize;
    let tag = field(d, value, 1)?;
    (d.get(tag) == Some(&4)).then_some(())?;
    field(d, value, 0)
}

/// Every ball in a `world_item` scene, in file order. Fails if any `itemNo` key cannot be resolved,
/// so a layout change never leaves balls half-patched.
pub fn find(d: &[u8]) -> Result<Vec<Ball>, DataError> {
    let item_key = key("itemNo");
    let num_key = key("num");
    let mut balls = Vec::new();
    let mut from = 0;
    while let Some(found) = find_bytes(&d[from..], &item_key) {
        let string = from + found;
        from = string + item_key.len();
        let item_at = int_property(d, string)
            .ok_or_else(|| DataError::Shape(format!("item ball at {string:#x}: itemNo is not an integer property")))?;
        let item = i64_at(d, item_at)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or_else(|| DataError::Shape(format!("item ball at {string:#x}: item out of range")))?;
        let window = string.saturating_sub(NUM_REACH)..string;
        let num_at = find_bytes_last(&d[window.clone()], &num_key).and_then(|at| int_property(d, window.start + at));
        let num = num_at.and_then(|at| i64_at(d, at)).unwrap_or(1);
        balls.push(Ball { item_at, item, num_at, num });
    }
    Ok(balls)
}

/// Writes a ball's new item and, when the ball stores one, stack size.
pub fn write(d: &mut [u8], ball: &Ball, item: u16, num: i64) {
    d[ball.item_at..ball.item_at + 8].copy_from_slice(&i64::from(item).to_le_bytes());
    if let Some(at) = ball.num_at {
        d[at..at + 8].copy_from_slice(&num.to_le_bytes());
    }
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn find_bytes_last(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

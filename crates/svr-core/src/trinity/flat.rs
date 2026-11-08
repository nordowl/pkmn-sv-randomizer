#[derive(Debug, thiserror::Error)]
#[error("invalid flatbuffer: {0}")]
pub(crate) struct FbError(String);

#[derive(Clone, Copy)]
pub(crate) struct FbVector {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// Bounds-checked reader for the few flatbuffer shapes the Trinity containers use.
#[derive(Clone, Copy)]
pub(crate) struct Fb<'a> {
    buf: &'a [u8],
}

impl<'a> Fb<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Fb<'a> {
        Fb { buf }
    }

    fn array<const N: usize>(&self, pos: usize) -> Result<[u8; N], FbError> {
        pos.checked_add(N)
            .and_then(|end| self.buf.get(pos..end))
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| FbError(format!("read of {N} bytes at {pos:#x} is out of bounds")))
    }

    fn u16_at(&self, pos: usize) -> Result<u16, FbError> {
        self.array(pos).map(u16::from_le_bytes)
    }

    fn u32_at(&self, pos: usize) -> Result<u32, FbError> {
        self.array(pos).map(u32::from_le_bytes)
    }

    fn forward(&self, pos: usize) -> Result<usize, FbError> {
        Ok(pos + self.u32_at(pos)? as usize)
    }

    pub(crate) fn root(&self) -> Result<usize, FbError> {
        self.forward(0)
    }

    fn field(&self, table: usize, slot: usize) -> Result<Option<usize>, FbError> {
        let back = i64::from(self.array(table).map(i32::from_le_bytes)?);
        let vtable = usize::try_from(table as i64 - back)
            .map_err(|_| FbError(format!("vtable of table at {table:#x} is out of bounds")))?;
        let entry = 4 + 2 * slot;
        if entry >= usize::from(self.u16_at(vtable)?) {
            return Ok(None);
        }
        let offset = usize::from(self.u16_at(vtable + entry)?);
        Ok((offset != 0).then_some(table + offset))
    }

    fn vector(&self, table: usize, slot: usize, element: usize) -> Result<FbVector, FbError> {
        let Some(pos) = self.field(table, slot)? else {
            return Ok(FbVector { start: 0, len: 0 });
        };
        let header = self.forward(pos)?;
        let len = self.u32_at(header)? as usize;
        let start = header + 4;
        if start + len * element > self.buf.len() {
            return Err(FbError(format!("vector at {header:#x} is out of bounds")));
        }
        Ok(FbVector { start, len })
    }

    pub(crate) fn u64_vector(&self, table: usize, slot: usize) -> Result<FbVector, FbError> {
        self.vector(table, slot, 8)
    }

    pub(crate) fn offset_vector(&self, table: usize, slot: usize) -> Result<FbVector, FbError> {
        self.vector(table, slot, 4)
    }

    pub(crate) fn u64s(&self, vector: FbVector) -> impl Iterator<Item = u64> + 'a {
        let bytes = self.buf.get(vector.start..vector.start + vector.len * 8);
        let (chunks, _) = bytes.unwrap_or_default().as_chunks::<8>();
        chunks.iter().map(|chunk| u64::from_le_bytes(*chunk))
    }

    pub(crate) fn offsets(&self, vector: FbVector) -> Result<Vec<usize>, FbError> {
        (0..vector.len)
            .map(|i| self.forward(vector.start + i * 4))
            .collect()
    }

    pub(crate) fn string_at(&self, pos: usize) -> Result<&'a str, FbError> {
        let len = self.u32_at(pos)? as usize;
        self.buf
            .get(pos + 4..pos + 4 + len)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or_else(|| FbError(format!("bad string at {pos:#x}")))
    }

    pub(crate) fn u64_field(&self, table: usize, slot: usize, default: u64) -> Result<u64, FbError> {
        match self.field(table, slot)? {
            Some(pos) => self.array(pos).map(u64::from_le_bytes),
            None => Ok(default),
        }
    }

    pub(crate) fn i8_field(&self, table: usize, slot: usize, default: i8) -> Result<i8, FbError> {
        match self.field(table, slot)? {
            Some(pos) => self.array(pos).map(i8::from_le_bytes),
            None => Ok(default),
        }
    }

    pub(crate) fn bytes_field(&self, table: usize, slot: usize) -> Result<&'a [u8], FbError> {
        let vector = self.vector(table, slot, 1)?;
        Ok(&self.buf[vector.start..vector.start + vector.len])
    }
}

#[cfg(test)]
pub(crate) mod build {
    pub(crate) enum Field {
        Absent,
        U64(u64),
        I8(i8),
        Ref(Node),
    }

    pub(crate) enum Node {
        Table(Vec<Field>),
        U64s(Vec<u64>),
        Bytes(Vec<u8>),
        Str(String),
        Refs(Vec<Node>),
    }

    pub(crate) fn buffer(root: &Node) -> Vec<u8> {
        let mut out = vec![0; 4];
        let pos = write(&mut out, root);
        out[..4].copy_from_slice(&(pos as u32).to_le_bytes());
        out
    }

    fn patch_offset(out: &mut [u8], slot: usize, target: usize) {
        out[slot..slot + 4].copy_from_slice(&((target - slot) as u32).to_le_bytes());
    }

    fn write(out: &mut Vec<u8>, node: &Node) -> usize {
        let pos = out.len();
        match node {
            Node::Table(fields) => return write_table(out, fields),
            Node::U64s(values) => {
                out.extend((values.len() as u32).to_le_bytes());
                values.iter().for_each(|v| out.extend(v.to_le_bytes()));
            }
            Node::Bytes(bytes) => {
                out.extend((bytes.len() as u32).to_le_bytes());
                out.extend(bytes);
            }
            Node::Str(text) => {
                out.extend((text.len() as u32).to_le_bytes());
                out.extend(text.as_bytes());
            }
            Node::Refs(nodes) => {
                out.extend((nodes.len() as u32).to_le_bytes());
                out.resize(out.len() + nodes.len() * 4, 0);
                for (i, child) in nodes.iter().enumerate() {
                    let target = write(out, child);
                    patch_offset(out, pos + 4 + i * 4, target);
                }
            }
        }
        pos
    }

    fn write_table(out: &mut Vec<u8>, fields: &[Field]) -> usize {
        let mut slots = Vec::new();
        let mut size = 4usize;
        for field in fields {
            let width = match field {
                Field::Absent => 0,
                Field::U64(_) => 8,
                Field::I8(_) => 1,
                Field::Ref(_) => 4,
            };
            slots.push(if width == 0 { 0 } else { size });
            size += width;
        }

        let vtable = out.len();
        out.extend(((4 + 2 * fields.len()) as u16).to_le_bytes());
        out.extend((size as u16).to_le_bytes());
        slots.iter().for_each(|slot| out.extend((*slot as u16).to_le_bytes()));

        let table = out.len();
        out.extend(((table - vtable) as i32).to_le_bytes());
        out.resize(table + size, 0);
        for (field, slot) in fields.iter().zip(&slots) {
            let at = table + slot;
            match field {
                Field::Absent => {}
                Field::U64(value) => out[at..at + 8].copy_from_slice(&value.to_le_bytes()),
                Field::I8(value) => out[at] = *value as u8,
                Field::Ref(child) => {
                    let target = write(out, child);
                    patch_offset(out, at, target);
                }
            }
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::build::{buffer, Field, Node};
    use super::*;

    #[test]
    fn reads_fields_and_applies_defaults() {
        let raw = buffer(&Node::Table(vec![
            Field::Ref(Node::U64s(vec![1, 2])),
            Field::Absent,
            Field::I8(-1),
            Field::U64(9),
            Field::Ref(Node::Bytes(b"xyz".to_vec())),
            Field::Ref(Node::Refs(vec![Node::Str("hello".to_owned())])),
        ]));
        let fb = Fb::new(&raw);
        let root = fb.root().unwrap();
        let hashes = fb.u64_vector(root, 0).unwrap();
        assert_eq!(fb.u64s(hashes).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(fb.u64_field(root, 1, 5).unwrap(), 5);
        assert_eq!(fb.i8_field(root, 2, 0).unwrap(), -1);
        assert_eq!(fb.u64_field(root, 3, 0).unwrap(), 9);
        assert_eq!(fb.bytes_field(root, 4).unwrap(), b"xyz");
        let strings = fb.offset_vector(root, 5).unwrap();
        let first = fb.offsets(strings).unwrap()[0];
        assert_eq!(fb.string_at(first).unwrap(), "hello");
        assert_eq!(fb.u64_field(root, 40, 7).unwrap(), 7);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        for garbage in [vec![], vec![0xFF; 3], vec![0xFF; 64], vec![0x04, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0x7F]] {
            let fb = Fb::new(&garbage);
            let outcome = fb.root().and_then(|root| fb.u64_vector(root, 0));
            assert!(outcome.is_err());
        }
    }
}

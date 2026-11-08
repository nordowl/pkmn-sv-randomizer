use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WireError {
    #[error("read of {len} bytes at offset {pos} is outside the {size}-byte buffer")]
    OutOfBounds { pos: usize, len: usize, size: usize },
    #[error("uoffset at {pos} is zero, which points at itself")]
    ZeroOffset { pos: usize },
    #[error("table at {table} has a bad vtable: {reason}")]
    BadVtable { table: usize, reason: &'static str },
    #[error("string at {pos} is not valid UTF-8")]
    InvalidUtf8 { pos: usize },
}

#[derive(Clone, Copy)]
pub(super) struct Buf<'a>(pub(super) &'a [u8]);

impl<'a> Buf<'a> {
    fn out_of_bounds(self, pos: usize, len: usize) -> WireError {
        WireError::OutOfBounds { pos, len, size: self.0.len() }
    }

    pub(super) fn bytes(self, pos: usize, len: usize) -> Result<&'a [u8], WireError> {
        pos.checked_add(len)
            .and_then(|end| self.0.get(pos..end))
            .ok_or_else(|| self.out_of_bounds(pos, len))
    }

    pub(super) fn array<const N: usize>(self, pos: usize) -> Result<[u8; N], WireError> {
        self.0
            .get(pos..)
            .and_then(|rest| rest.first_chunk::<N>())
            .copied()
            .ok_or_else(|| self.out_of_bounds(pos, N))
    }

    pub(super) fn u32(self, pos: usize) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.array(pos)?))
    }

    pub(super) fn offset_target(self, pos: usize) -> Result<usize, WireError> {
        match self.u32(pos)? {
            0 => Err(WireError::ZeroOffset { pos }),
            offset => pos.checked_add(offset as usize).ok_or_else(|| self.out_of_bounds(pos, 4)),
        }
    }

    pub(super) fn string(self, pos: usize) -> Result<&'a str, WireError> {
        let len = self.u32(pos)? as usize;
        let start = pos + 4;
        std::str::from_utf8(self.bytes(start, len)?).map_err(|_| WireError::InvalidUtf8 { pos })
    }

    /// Returns the element data position and count of the vector whose length prefix is at `pos`.
    pub(super) fn vector(self, pos: usize, stride: usize) -> Result<(usize, usize), WireError> {
        let count = self.u32(pos)? as usize;
        let data = pos + 4;
        let len = count.checked_mul(stride).ok_or_else(|| self.out_of_bounds(data, usize::MAX))?;
        self.bytes(data, len)?;
        Ok((data, count))
    }

    pub(super) fn table(self, pos: usize) -> Result<TableRef<'a>, WireError> {
        let soffset = i32::from_le_bytes(self.array(pos)?);
        let vtable = (pos as i64)
            .checked_sub(i64::from(soffset))
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(WireError::BadVtable { table: pos, reason: "position before buffer start" })?;
        let vtable_len = usize::from(u16::from_le_bytes(self.array(vtable)?));
        if vtable_len < 4 || vtable_len % 2 != 0 {
            return Err(WireError::BadVtable { table: pos, reason: "invalid vtable size" });
        }
        self.bytes(vtable, vtable_len)?;
        Ok(TableRef { buf: self, pos, vtable, vtable_len })
    }
}

#[derive(Clone, Copy)]
pub(super) struct TableRef<'a> {
    buf: Buf<'a>,
    pos: usize,
    vtable: usize,
    vtable_len: usize,
}

impl<'a> TableRef<'a> {
    pub(super) fn slot_count(self) -> usize {
        (self.vtable_len - 4) / 2
    }

    /// Returns the absolute position of the field in `slot`, or `None` when it is absent.
    pub(super) fn field(self, slot: usize) -> Result<Option<usize>, WireError> {
        if slot >= self.slot_count() {
            return Ok(None);
        }
        let offset = u16::from_le_bytes(self.buf.array(self.vtable + 4 + 2 * slot)?);
        Ok((offset != 0).then_some(self.pos + usize::from(offset)))
    }

    pub(super) fn scalar<const N: usize>(self, slot: usize) -> Result<Option<[u8; N]>, WireError> {
        self.field(slot)?.map(|pos| self.buf.array(pos)).transpose()
    }

    pub(super) fn string_field(self, slot: usize) -> Result<Option<&'a str>, WireError> {
        self.field(slot)?
            .map(|pos| self.buf.string(self.buf.offset_target(pos)?))
            .transpose()
    }

    pub(super) fn table_field(self, slot: usize) -> Result<Option<TableRef<'a>>, WireError> {
        self.field(slot)?
            .map(|pos| self.buf.table(self.buf.offset_target(pos)?))
            .transpose()
    }

    pub(super) fn tables_field(self, slot: usize) -> Result<Vec<TableRef<'a>>, WireError> {
        let Some(pos) = self.field(slot)? else {
            return Ok(Vec::new());
        };
        let (data, count) = self.buf.vector(self.buf.offset_target(pos)?, 4)?;
        (0..count)
            .map(|i| self.buf.table(self.buf.offset_target(data + 4 * i)?))
            .collect()
    }
}

use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

pub trait Storage: Send + Sync {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read_vec(&self, offset: u64, size: u64) -> io::Result<Vec<u8>> {
        check_range(offset, size, self.len())?;
        let size = usize::try_from(size)
            .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "read larger than memory"))?;
        let mut buf = vec![0; size];
        self.read_at(offset, &mut buf)?;
        Ok(buf)
    }
}

pub(crate) fn check_range(offset: u64, size: u64, len: u64) -> io::Result<()> {
    match offset.checked_add(size) {
        Some(end) if end <= len => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("read of {size:#x} bytes at {offset:#x} goes past the end ({len:#x})"),
        )),
    }
}

pub struct FileStorage {
    file: File,
    len: u64,
}

impl FileStorage {
    pub fn open(path: &Path) -> io::Result<FileStorage> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(FileStorage { file, len })
    }
}

impl Storage for FileStorage {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        read_exact_at(&self.file, offset, buf)
    }

    fn len(&self) -> u64 {
        self.len
    }
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut offset: u64, mut buf: &mut [u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let read = file.seek_read(buf, offset)?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[read..];
        offset += read as u64;
    }
    Ok(())
}

#[cfg(unix)]
fn read_exact_at(file: &File, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

impl Storage for Vec<u8> {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        check_range(offset, buf.len() as u64, self.len() as u64)?;
        let start = offset as usize;
        buf.copy_from_slice(&self[start..start + buf.len()]);
        Ok(())
    }

    fn len(&self) -> u64 {
        Vec::len(self) as u64
    }
}

pub struct SubStorage {
    inner: Arc<dyn Storage>,
    offset: u64,
    len: u64,
}

impl SubStorage {
    pub fn new(inner: Arc<dyn Storage>, offset: u64, len: u64) -> io::Result<SubStorage> {
        check_range(offset, len, inner.len())?;
        Ok(SubStorage { inner, offset, len })
    }
}

impl Storage for SubStorage {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        check_range(offset, buf.len() as u64, self.len)?;
        self.inner.read_at(self.offset + offset, buf)
    }

    fn len(&self) -> u64 {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_storage_refuses_to_escape_its_range() {
        let inner: Arc<dyn Storage> = Arc::new((0u8..100).collect::<Vec<u8>>());
        assert!(SubStorage::new(inner.clone(), 90, 11).is_err());
        let sub = SubStorage::new(inner, 10, 20).unwrap();
        assert_eq!(sub.read_vec(5, 2).unwrap(), vec![15, 16]);
        assert!(sub.read_vec(19, 2).is_err());
        assert!(sub.read_vec(u64::MAX, 2).is_err());
    }
}

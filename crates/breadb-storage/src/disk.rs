use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use breadb_core::{DbError, PAGE_SIZE, PageId, Result};
use parking_lot::Mutex;

pub struct DiskManager {
    inner: Mutex<DiskInner>,
}

struct DiskInner {
    file: File,
    num_pages: u32,
}

impl DiskManager {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        let file_len = file.metadata()?.len();
        if file_len % (PAGE_SIZE as u64) != 0 {
            return Err(DbError::Corruption(format!(
                "Database file size ({file_len} bytes) is not a multiple of PAGE_SIZE ({PAGE_SIZE})"
            )));
        }

        let num_pages = (file_len / (PAGE_SIZE as u64)) as u32;
        Ok(Self {
            inner: Mutex::new(DiskInner { file, num_pages }),
        })
    }

    pub fn allocate_page(&self) -> Result<PageId> {
        let mut guard = self.inner.lock();
        let page_id = PageId(guard.num_pages);
        let offset = u64::from(page_id.0) * (PAGE_SIZE as u64);

        guard.file.seek(SeekFrom::Start(offset))?;
        guard.file.write_all(&[0u8; PAGE_SIZE])?;
        guard.file.flush()?;

        guard.num_pages += 1;
        Ok(page_id)
    }

    pub fn read_page(&self, page_id: PageId, buf: &mut [u8; PAGE_SIZE]) -> Result<()> {
        let mut guard = self.inner.lock();
        if page_id.0 >= guard.num_pages {
            return Err(DbError::PageNotFound(page_id));
        }

        let offset = u64::from(page_id.0) * (PAGE_SIZE as u64);
        guard.file.seek(SeekFrom::Start(offset))?;
        guard.file.read_exact(buf)?;

        Ok(())
    }

    pub fn write_page(&self, page_id: PageId, buf: &mut [u8; PAGE_SIZE]) -> Result<()> {
        let mut guard = self.inner.lock();
        if page_id.0 >= guard.num_pages {
            return Err(DbError::PageNotFound(page_id));
        }

        let offset = u64::from(page_id.0) * (PAGE_SIZE as u64);
        guard.file.seek(SeekFrom::Start(offset))?;
        guard.file.write_all(buf)?;

        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        let guard = self.inner.lock();
        guard.file.sync_all()?;

        Ok(())
    }

    pub fn num_pages(&self) -> u32 {
        self.inner.lock().num_pages
    }
}

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use breadb_core::{DbError, PAGE_SIZE, PageId, Result};
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::disk::DiskManager;

type FrameId = usize;

struct Frame {
    page_id: Mutex<Option<PageId>>,
    pin_count: AtomicU32,
    is_dirty: AtomicBool,
    ref_bit: AtomicBool,
    data: RwLock<[u8; PAGE_SIZE]>,
}

impl Frame {
    fn new() -> Self {
        Self {
            page_id: Mutex::new(None),
            pin_count: AtomicU32::new(0),
            is_dirty: AtomicBool::new(false),
            ref_bit: AtomicBool::new(false),
            data: RwLock::new([0u8; PAGE_SIZE]),
        }
    }
}

struct PoolMeta {
    page_table: HashMap<PageId, FrameId>,
    free_frames: Vec<FrameId>,
    clock_hand: usize,
}

pub struct BufferPoolManager {
    disk: Arc<DiskManager>,
    frames: Vec<Arc<Frame>>,
    meta: Mutex<PoolMeta>,
}

impl BufferPoolManager {
    pub fn new(pool_size: usize, disk: Arc<DiskManager>) -> Self {
        assert!(pool_size > 0, "Buffer pool size must be greater than 0");
        let frames = (0..pool_size).map(|_| Arc::new(Frame::new())).collect();
        let free_frames = (0..pool_size).rev().collect();

        Self {
            disk,
            frames,
            meta: Mutex::new(PoolMeta {
                page_table: HashMap::with_capacity(pool_size),
                free_frames,
                clock_hand: 0,
            }),
        }
    }

    fn acquire_frame(&self, meta: &mut PoolMeta) -> Result<FrameId> {
        if let Some(frame_id) = meta.free_frames.pop() {
            return Ok(frame_id);
        }

        let n = self.frames.len();
        for _ in 0..(2 * n) {
            let candidate_id = meta.clock_hand;
            meta.clock_hand = (meta.clock_hand + 1) % n;
            let frame = &self.frames[candidate_id];

            if frame.pin_count.load(Ordering::Acquire) > 0 {
                continue;
            }

            if frame.ref_bit.swap(false, Ordering::AcqRel) {
                continue;
            }

            let mut old_page_guard = frame.page_id.lock();
            if let Some(old_page_id) = *old_page_guard {
                if frame.is_dirty.load(Ordering::Acquire) {
                    let mut data = frame.data.write();
                    self.disk.write_page(old_page_id, &mut data)?;
                    frame.is_dirty.store(false, Ordering::Release);
                }
                *old_page_guard = None;
                meta.page_table.remove(&old_page_id);
            }
            return Ok(candidate_id);
        }

        Err(DbError::BufferPoolFull)
    }

    fn pin_frame(&self, page_id: PageId, is_new: bool) -> Result<Arc<Frame>> {
        let mut meta = self.meta.lock();

        if let Some(&frame_id) = meta.page_table.get(&page_id) {
            let frame = Arc::clone(&self.frames[frame_id]);
            frame.pin_count.fetch_add(1, Ordering::AcqRel);
            frame.ref_bit.store(true, Ordering::Release);
            return Ok(frame);
        }

        let frame_id = self.acquire_frame(&mut meta)?;
        let frame = Arc::clone(&self.frames[frame_id]);

        {
            let mut data = frame.data.write();
            if is_new {
                data.fill(0);
            } else {
                self.disk.read_page(page_id, &mut data)?;
            }
        }

        *frame.page_id.lock() = Some(page_id);
        frame.pin_count.store(1, Ordering::Release);
        frame.is_dirty.store(is_new, Ordering::Release);
        frame.ref_bit.store(true, Ordering::Release);
        meta.page_table.insert(page_id, frame_id);

        Ok(frame)
    }

    pub fn fetch_page_read(&self, page_id: PageId) -> Result<PageReadGuard> {
        let frame = self.pin_frame(page_id, false)?;
        let guard = unsafe {
            std::mem::transmute::<
                RwLockReadGuard<'_, [u8; PAGE_SIZE]>,
                RwLockReadGuard<'static, [u8; PAGE_SIZE]>,
            >(frame.data.read())
        };
        Ok(PageReadGuard { frame, guard })
    }

    pub fn fetch_page_write(&self, page_id: PageId) -> Result<PageWriteGuard> {
        let frame = self.pin_frame(page_id, false)?;
        let guard = unsafe {
            std::mem::transmute::<
                RwLockWriteGuard<'_, [u8; PAGE_SIZE]>,
                RwLockWriteGuard<'static, [u8; PAGE_SIZE]>,
            >(frame.data.write())
        };
        frame.is_dirty.store(true, Ordering::Release);
        Ok(PageWriteGuard { frame, guard })
    }

    pub fn new_page(&self) -> Result<(PageId, PageWriteGuard)> {
        let page_id = self.disk.allocate_page()?;
        let frame = self.pin_frame(page_id, true)?;
        let guard = unsafe {
            std::mem::transmute::<
                RwLockWriteGuard<'_, [u8; PAGE_SIZE]>,
                RwLockWriteGuard<'static, [u8; PAGE_SIZE]>,
            >(frame.data.write())
        };
        Ok((page_id, PageWriteGuard { frame, guard }))
    }

    pub fn flush_page(&self, page_id: PageId) -> Result<()> {
        let frame = {
            let meta = self.meta.lock();
            meta.page_table
                .get(&page_id)
                .map(|&id| Arc::clone(&self.frames[id]))
        };

        if let Some(frame) = frame {
            let mut data = frame.data.write();
            if frame.is_dirty.load(Ordering::Acquire) {
                self.disk.write_page(page_id, &mut data)?;
                frame.is_dirty.store(false, Ordering::Release);
            }
        }
        Ok(())
    }

    pub fn flush_all(&self) -> Result<()> {
        for frame in &self.frames {
            let page_id = *frame.page_id.lock();
            if let Some(pid) = page_id {
                let mut data = frame.data.write();
                if frame.is_dirty.load(Ordering::Acquire) {
                    self.disk.write_page(pid, &mut data)?;
                    frame.is_dirty.store(false, Ordering::Release);
                }
            }
        }
        self.disk.sync()
    }
}

pub struct PageReadGuard {
    guard: RwLockReadGuard<'static, [u8; PAGE_SIZE]>,
    frame: Arc<Frame>,
}

impl Deref for PageReadGuard {
    type Target = [u8; PAGE_SIZE];
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl Drop for PageReadGuard {
    fn drop(&mut self) {
        self.frame.pin_count.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct PageWriteGuard {
    guard: RwLockWriteGuard<'static, [u8; PAGE_SIZE]>,
    frame: Arc<Frame>,
}

impl Deref for PageWriteGuard {
    type Target = [u8; PAGE_SIZE];
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl DerefMut for PageWriteGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for PageWriteGuard {
    fn drop(&mut self) {
        self.frame.pin_count.fetch_sub(1, Ordering::AcqRel);
    }
}

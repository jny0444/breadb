use breadb_core::{DbError, Lsn, PAGE_SIZE, PageId, Result, SlotId};
use byteorder::{ByteOrder, LittleEndian};

const HEADER_SIZE: usize = 20;
const SLOT_SIZE: usize = 4;

// offsets
const OFF_PAGE_ID: usize = 0;
const OFF_NEXT_PAGE_ID: usize = 4;
const OFF_LSN: usize = 8;
const OFF_SLOT_COUNT: usize = 16;
const OFF_FREE_END: usize = 18;

/*
*
* SlottedPage Layout
*
* *****************************************************************************
* |                                 Header 20B                                |
* | page_id 4B | next_page_id 4B | lsn 8B | slot_count 2B | free_space_end 2B |
* |---------------------------------------+-----------------------------------|
* |                             offset 2B | length 2B                         |
* *****************************************************************************
*
*/

pub struct SlottedPage<T> {
    data: T,
}

impl<T: AsRef<[u8]>> SlottedPage<T> {
    pub fn wrap(data: T) -> Self {
        assert_eq!(
            data.as_ref().len(),
            PAGE_SIZE,
            "Slotted page must be PAGE_SIZE bytes"
        );
        Self { data }
    }

    #[inline]
    pub fn page_id(&self) -> PageId {
        PageId(LittleEndian::read_u32(&self.data.as_ref()[OFF_PAGE_ID..]))
    }

    #[inline]
    pub fn next_page_id(&self) -> PageId {
        PageId(LittleEndian::read_u32(
            &self.data.as_ref()[OFF_NEXT_PAGE_ID..],
        ))
    }

    #[inline]
    pub fn lsn(&self) -> Lsn {
        Lsn(LittleEndian::read_u64(&self.data.as_ref()[OFF_LSN..]))
    }

    #[inline]
    pub fn slot_count(&self) -> u16 {
        LittleEndian::read_u16(&self.data.as_ref()[OFF_SLOT_COUNT..])
    }

    #[inline]
    fn free_space_end(&self) -> u16 {
        LittleEndian::read_u16(&self.data.as_ref()[OFF_FREE_END..])
    }

    #[inline]
    fn free_space_start(&self) -> usize {
        HEADER_SIZE + (self.slot_count() as usize) * SLOT_SIZE
    }

    #[inline]
    pub fn free_space(&self) -> usize {
        (self.free_space_end() as usize).saturating_sub(self.free_space_start())
    }

    fn read_slot(&self, slot_id: SlotId) -> Result<(u16, u16)> {
        if slot_id.0 >= self.slot_count() {
            return Err(DbError::Corruption(format!(
                "Slot {} out of bounds (count {})",
                slot_id.0,
                self.slot_count()
            )));
        }
        let pos = HEADER_SIZE + (slot_id.0 as usize) * SLOT_SIZE;
        let bytes = self.data.as_ref();
        let offset = LittleEndian::read_u16(&bytes[pos..pos + 2]);
        let len = LittleEndian::read_u16(&bytes[pos + 2..pos + 4]);
        Ok((offset, len))
    }

    pub fn get(&self, slot_id: SlotId) -> Result<Option<&[u8]>> {
        let (offset, len) = self.read_slot(slot_id)?;
        if len == 0 && offset == 0 {
            return Ok(None);
        }
        let start = offset as usize;
        let end = start + (len as usize);
        if start < self.free_space_end() as usize || end > PAGE_SIZE {
            return Err(DbError::Corruption("Tuple offset exceeds PAGE_SIZE".into()));
        }
        Ok(Some(&self.data.as_ref()[start..end]))
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> SlottedPage<T> {
    pub fn init(&mut self, page_id: PageId) {
        let buf = self.data.as_mut();
        buf.fill(0);
        LittleEndian::write_u32(&mut buf[OFF_PAGE_ID..], page_id.0);
        LittleEndian::write_u32(&mut buf[OFF_NEXT_PAGE_ID..], PageId::INVALID.0);
        LittleEndian::write_u64(&mut buf[OFF_LSN..], Lsn::INVALID.0);
        LittleEndian::write_u16(&mut buf[OFF_SLOT_COUNT..], 0);
        LittleEndian::write_u16(&mut buf[OFF_FREE_END..], PAGE_SIZE as u16);
    }

    pub fn set_next_page_id(&mut self, next: PageId) {
        LittleEndian::write_u32(&mut self.data.as_mut()[OFF_NEXT_PAGE_ID..], next.0);
    }

    pub fn set_lsn(&mut self, lsn: Lsn) {
        LittleEndian::write_u64(&mut self.data.as_mut()[OFF_LSN..], lsn.0);
    }

    fn write_slot(&mut self, slot_id: SlotId, offset: u16, len: u16) {
        let pos = HEADER_SIZE + (slot_id.0 as usize) * SLOT_SIZE;
        let buf = self.data.as_mut();
        LittleEndian::write_u16(&mut buf[pos..pos + 2], offset);
        LittleEndian::write_u16(&mut buf[pos + 2..pos + 4], len);
    }

    fn compact(&mut self) {
        let mut packed = [0; PAGE_SIZE];
        let mut free_end = PAGE_SIZE;

        for idx in 0..self.slot_count() {
            let slot_id = SlotId(idx);
            let (offset, len) = self.read_slot(slot_id).expect("slot index is in bounds");
            if len == 0 && offset == 0 {
                continue;
            }

            let start = offset as usize;
            let end = start + len as usize;
            free_end -= len as usize;
            packed[free_end..free_end + len as usize]
                .copy_from_slice(&self.data.as_ref()[start..end]);
            self.write_slot(slot_id, free_end as u16, len);
        }

        self.data.as_mut()[free_end..].copy_from_slice(&packed[free_end..]);
        LittleEndian::write_u16(&mut self.data.as_mut()[OFF_FREE_END..], free_end as u16);
    }

    pub fn insert(&mut self, tuple_bytes: &[u8]) -> Result<SlotId> {
        let len = u16::try_from(tuple_bytes.len()).map_err(|_| DbError::PageFull {
            page_id: self.page_id(),
            needed: tuple_bytes.len(),
            available: self.free_space(),
        })?;

        let slot_count = self.slot_count();
        let mut reusable_slot = None;
        for idx in 0..slot_count {
            let (_, slot_len) = self.read_slot(SlotId(idx))?;
            if slot_len == 0 {
                reusable_slot = Some(SlotId(idx));
                break;
            }
        }

        let slot_overhead = if reusable_slot.is_some() {
            0
        } else {
            SLOT_SIZE
        };
        let needed = (len as usize) + slot_overhead;
        let available = self.free_space();

        if needed > available {
            return Err(DbError::PageFull {
                page_id: self.page_id(),
                needed,
                available,
            });
        }

        let new_free_end = self.free_space_end() - len;
        let start = new_free_end as usize;
        let end = start + (len as usize);

        self.data.as_mut()[start..end].copy_from_slice(tuple_bytes);
        LittleEndian::write_u16(&mut self.data.as_mut()[OFF_FREE_END..], new_free_end);

        let assigned_role = match reusable_slot {
            Some(slot) => slot,
            None => {
                let slot = SlotId(slot_count);
                LittleEndian::write_u16(&mut self.data.as_mut()[OFF_SLOT_COUNT..], slot_count + 1);
                slot
            }
        };

        self.write_slot(assigned_role, new_free_end, len);
        Ok(assigned_role)
    }

    pub fn delete(&mut self, slot_id: SlotId) -> Result<()> {
        let (offset, len) = self.read_slot(slot_id)?;
        if len == 0 && offset == 0 {
            return Err(DbError::Corruption(format!(
                "Slot {} is already deleted",
                slot_id.0
            )));
        }
        self.write_slot(slot_id, 0, 0);
        self.compact();
        Ok(())
    }
}

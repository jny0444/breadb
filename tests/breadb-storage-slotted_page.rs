use breadb_core::{Tuple, Value};
use breadb_storage::{BufferPoolManager, DiskManager, SlottedPage};
use std::sync::Arc;
use tempfile::NamedTempFile;

#[test]
fn test_buffer_pool_persistence_and_eviction() {
    let tmp = NamedTempFile::new().unwrap();
    let disk = Arc::new(DiskManager::open(tmp.path()).unwrap());

    let bpm = BufferPoolManager::new(2, Arc::clone(&disk));

    let mut page_ids = Vec::new();
    for i in 0..4 {
        let (pid, mut guard) = bpm.new_page().unwrap();
        let mut page = SlottedPage::wrap(&mut *guard);
        page.init(pid);

        let tuple = Tuple::new(vec![Value::Integer(i), Value::Text(format!("row-{i}"))]);
        let slot = page.insert(&tuple.to_bytes().unwrap()).unwrap();
        assert_eq!(slot.0, 0);
        page_ids.push(pid);
    }

    bpm.flush_all().unwrap();

    for (i, &pid) in page_ids.iter().enumerate() {
        let guard = bpm.fetch_page_read(pid).unwrap();
        let page = SlottedPage::wrap(&*guard);
        let raw = page.get(breadb_core::SlotId(0)).unwrap().unwrap();
        let decoded = Tuple::from_bytes(raw).unwrap();
        assert_eq!(decoded.values[0], Value::Integer(i as i64));
    }
}

#[test]
fn test_delete_reclaims_space_and_reuses_slot() {
    let mut data = [0; breadb_core::PAGE_SIZE];
    let mut page = SlottedPage::wrap(&mut data);
    page.init(breadb_core::PageId(1));

    let first = page.insert(&vec![1; 2_000]).unwrap();
    page.insert(&vec![2; 2_000]).unwrap();
    page.delete(first).unwrap();
    assert_eq!(page.insert(&vec![3; 2_000]).unwrap(), first);
}

#[test]
fn test_empty_tuple_is_not_treated_as_deleted() {
    let mut data = [0; breadb_core::PAGE_SIZE];
    let mut page = SlottedPage::wrap(&mut data);
    page.init(breadb_core::PageId(1));

    let slot = page.insert(&[]).unwrap();
    assert_eq!(page.get(slot).unwrap(), Some([].as_slice()));
}

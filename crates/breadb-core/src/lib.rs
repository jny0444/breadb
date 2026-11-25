pub mod error;
pub mod ids;
pub mod schema;
pub mod types;

pub use error::{DbError, Result};
pub use ids::{Lsn, PageId, RowId, SlotId, TableId, TxnId};
pub use schema::{Column, Schema, Tuple};
pub use types::{DataType, Value};

pub const PAGE_SIZE: usize = 4096;

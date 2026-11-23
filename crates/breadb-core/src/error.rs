use thiserror::Error;

use crate::{
    ids::{PageId, RowId, TxnId},
    types::DataType,
};

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Page {0} not found")]
    PageNotFound(PageId),

    #[error("Buffer pool if full: all frames are currently filled")]
    BufferPoolFull,

    #[error(
        "Page {page_id} has insufficient free space (needed {needed}B, available {available}B)"
    )]
    PageFull {
        page_id: PageId,
        needed: usize,
        available: usize,
    },

    #[error("Invalid or deleted row at {0}")]
    InvalidRowId(RowId),

    #[error("Corrupted data detected: {0}")]
    Corruption(String),

    #[error("Table '{0}' already exists")]
    TableAlreadyExists(String),

    #[error("Table '{0}' does not exist")]
    TableNotFound(String),

    #[error("Column '{column}' not found in table '{table}'")]
    ColumnNotFound { table: String, column: String },

    #[error("Duplicate column '{0}' in schema")]
    DuplicateColumn(String),

    #[error("Type mismatch: expected {expected:?}, found {found:?}")]
    TypeMismatch { expected: DataType, found: DataType },

    #[error("Column '{0}' cannot be NULL")]
    NullConstraintViolation(String),

    #[error("Primary key / unique constraint violation on '{0}'")]
    UniqueConstraintViolation(String),

    #[error("Schema length mismatch: expected {expected} columns, got {got}")]
    SchemaLengthMismatch { expected: usize, got: usize },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Syntax error at position {pos}: {message}")]
    SyntaxError { pos: usize, message: String },

    #[error("Transaction {0} aborted: {1}")]
    TransactionAborted(TxnId, String),

    #[error("Deadlock detected for transaction {0}")]
    Deadlock(TxnId),

    #[error("Internal error: {0}")]
    Internal(String),
}

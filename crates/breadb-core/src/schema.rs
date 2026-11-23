use std::{collections::HashSet, io::Cursor};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    error::{DbError, Result},
    ids::RowId,
    types::{DataType, Value},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub is_primary_key: bool,
}

impl Column {
    pub fn new(name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable: true,
            is_primary_key: false,
        }
    }

    pub fn not_null(mut self) -> Self {
        self.nullable = false;
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.is_primary_key = true;
        self.nullable = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Schema {
    columns: Vec<Column>,
}

impl Schema {
    pub fn new(columns: Vec<Column>) -> Result<Self> {
        let mut seen = HashSet::with_capacity(columns.len());
        for col in &columns {
            if !seen.insert(col.name.to_lowercase()) {
                return Err(DbError::DuplicateColumn(col.name.clone()));
            }
        }

        Ok(Self { columns })
    }

    #[inline]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn insensitive_index_of(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn sensitive_index_of(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name.eq(name))
    }

    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    pub fn primary_key_index(&self) -> Option<usize> {
        self.columns.iter().position(|c| c.is_primary_key)
    }

    pub fn validate_values(&self, values: &[Value]) -> Result<()> {
        if values.len() != self.columns.len() {
            return Err(DbError::SchemaLengthMismatch {
                expected: self.columns.len(),
                got: values.len(),
            });
        }

        for (col, val) in self.columns.iter().zip(values.iter()) {
            if val.is_null() {
                if !col.nullable {
                    return Err(DbError::NullConstraintViolation(col.name.clone()));
                }
            } else {
                let found = val.data_type().unwrap();
                if found != col.data_type {
                    return Err(DbError::TypeMismatch {
                        expected: col.data_type,
                        found,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tuple {
    pub row_id: Option<RowId>,
    pub values: Vec<Value>,
}

impl Tuple {
    pub fn new(values: Vec<Value>) -> Self {
        Self {
            row_id: None,
            values,
        }
    }

    pub fn with_row_id(row_id: RowId, values: Vec<Value>) -> Self {
        Self {
            row_id: Some(row_id),
            values,
        }
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<&Value> {
        self.values.get(idx)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let count = u16::try_from(self.values.len())
            .map_err(|_| DbError::Corruption("Tuple column count exceeds u16::MAX".into()))?;

        buf.write_u16::<LittleEndian>(count)?;
        for val in &self.values {
            val.serialize(&mut buf)?;
        }

        Ok(buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let count = cursor.read_u16::<LittleEndian>()? as usize;

        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(Value::deserialize(&mut cursor)?);
        }

        Ok(Self::new(values))
    }
}

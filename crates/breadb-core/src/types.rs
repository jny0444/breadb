use std::{
    cmp::Ordering,
    fmt,
    io::{Cursor, Read, Write},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::{DbError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DataType {
    Boolean = 1,
    Integer = 2,
    Float = 3,
    Text = 4,
}

impl DataType {
    pub fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            1 => Ok(Self::Boolean),
            2 => Ok(Self::Integer),
            3 => Ok(Self::Float),
            4 => Ok(Self::Text),
            _ => Err(DbError::Corruption(format!("Unknown DataType tag: {tag}"))),
        }
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Boolean => write!(f, "BOOLEAN"),
            DataType::Integer => write!(f, "INT"),
            DataType::Float => write!(f, "FLOAT"),
            DataType::Text => write!(f, "TEXT"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

impl Value {
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Value::Null => None,
            Value::Boolean(_) => Some(DataType::Boolean),
            Value::Integer(_) => Some(DataType::Integer),
            Value::Float(_) => Some(DataType::Float),
            Value::Text(_) => Some(DataType::Text),
        }
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn matches_type(&self, expected: DataType, nullable: bool) -> Result<()> {
        match self {
            Value::Null => {
                if nullable {
                    Ok(())
                } else {
                    Err(DbError::NullConstraintViolation("column".into()))
                }
            }
            _ => {
                let found = self.data_type().unwrap();
                if found == expected {
                    Ok(())
                } else {
                    Err(DbError::TypeMismatch { expected, found })
                }
            }
        }
    }

    pub fn sql_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Null, _) | (_, Value::Null) => None,
            (Value::Boolean(a), Value::Boolean(b)) => Some(a.cmp(b)),
            (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Integer(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }

    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Value::Null => writer.write_u8(0)?,
            Value::Boolean(b) => {
                writer.write_u8(1)?;
                writer.write_u8(u8::from(*b))?;
            }
            Value::Integer(i) => {
                writer.write_u8(2)?;
                writer.write_i64::<LittleEndian>(*i)?;
            }
            Value::Float(f) => {
                writer.write_u8(3)?;
                writer.write_f64::<LittleEndian>(*f)?;
            }
            Value::Text(s) => {
                writer.write_u8(4)?;
                let bytes = s.as_bytes();
                let len = u32::try_from(bytes.len())
                    .map_err(|_| DbError::Corruption("Text value exceeds u32::MAX".into()))?;
                writer.write_u32::<LittleEndian>(len)?;
                writer.write_all(bytes)?;
            }
        }
        Ok(())
    }

    pub fn deserialize(cursor: &mut Cursor<&[u8]>) -> Result<Self> {
        let tag = cursor.read_u8()?;
        match tag {
            0 => Ok(Value::Null),
            1 => Ok(Value::Boolean(cursor.read_u8()? != 0)),
            2 => Ok(Value::Integer(cursor.read_i64::<LittleEndian>()?)),
            3 => Ok(Value::Float(cursor.read_f64::<LittleEndian>()?)),
            4 => {
                let len = cursor.read_u32::<LittleEndian>()? as usize;
                let mut buf = vec![0u8; len];
                cursor.read_exact(&mut buf)?;
                let s = String::from_utf8(buf)
                    .map_err(|e| DbError::Corruption(format!("Invalid UTF-8 in Text: {e}")))?;
                Ok(Value::Text(s))
            }
            _ => Err(DbError::Corruption(format!("Invalid Value tag: {tag}"))),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Text(s) => write!(f, "{s}"),
        }
    }
}

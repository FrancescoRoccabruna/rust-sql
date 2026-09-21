use crate::{
    DbError,
    table::{Column, Row, Value, ValueType},
};

const MYSQL_TYPE_DECIMAL: u8 = 0;
const MYSQL_TYPE_TINY: u8 = 1;
const MYSQL_TYPE_SHORT: u8 = 2;
const MYSQL_TYPE_LONG: u8 = 3;
const MYSQL_TYPE_FLOAT: u8 = 4;
const MYSQL_TYPE_DOUBLE: u8 = 5;
const MYSQL_TYPE_NULL: u8 = 6;
const MYSQL_TYPE_LONGLONG: u8 = 8;
const MYSQL_TYPE_INT24: u8 = 9;
const MYSQL_TYPE_DATE: u8 = 10;
const MYSQL_TYPE_TIME: u8 = 11;
const MYSQL_TYPE_DATETIME: u8 = 12;
const MYSQL_TYPE_YEAR: u8 = 13;
const MYSQL_TYPE_VARCHAR: u8 = 15;
const MYSQL_TYPE_BIT: u8 = 16;
const MYSQL_TYPE_TIMESTAMP: u8 = 17;
const MYSQL_TYPE_JSON: u8 = 245;
const MYSQL_TYPE_NEWDECIMAL: u8 = 246;
const MYSQL_TYPE_BLOB: u8 = 252;
const MYSQL_TYPE_VAR_STRING: u8 = 253;
const MYSQL_TYPE_STRING: u8 = 254;

const UNSIGNED_FLAG: u16 = 0x0020;

pub struct ResultParser {}

impl ResultParser {
    pub fn parse_column_count(payload: &[u8]) -> Result<usize, ParserError> {
        let mut offset = 0;

        let count = Self::read_lenenc_int(payload, &mut offset)?;

        let count =
            count.ok_or_else(|| ParserError::new(String::from("Invalid NULL column count")))?;

        usize::try_from(count)
            .map_err(|_| ParserError::new(String::from("Column count exceeds usize")))
    }

    pub fn parse_column_definition(payload: &[u8]) -> Result<(Column, u8, u16), ParserError> {
        let mut offset = 0;

        // catalog
        Self::read_lenenc_string(payload, &mut offset)?;

        // schema
        Self::read_lenenc_string(payload, &mut offset)?;

        // table
        Self::read_lenenc_string(payload, &mut offset)?;

        // original table
        Self::read_lenenc_string(payload, &mut offset)?;

        // column name
        let name = Self::read_lenenc_string(payload, &mut offset)?;

        // original column name
        Self::read_lenenc_string(payload, &mut offset)?;

        // length of fixed fields
        let fixed_fields_length = Self::read_u8(payload, &mut offset)?;

        if fixed_fields_length < 0x0c {
            return Err(ParserError::new(String::from(
                "Invalid MySQL column definition",
            )));
        }

        // character set
        let _character_set = Self::read_u16_le(payload, &mut offset)?;

        // column length
        let _column_length = Self::read_u32_le(payload, &mut offset)?;

        // MySQL type
        let column_type = Self::read_u8(payload, &mut offset)?;

        // flags
        let flags = Self::read_u16_le(payload, &mut offset)?;

        // decimals
        let _decimals = Self::read_u8(payload, &mut offset)?;

        // filler
        Self::read_bytes(payload, &mut offset, 2)?;

        let value_type = Self::value_type_from_mysql_type(column_type)?;

        let column = Column { name, value_type };

        Ok((column, column_type, flags))
    }

    pub fn parse_row(
        payload: &[u8],
        columns: &[Column],
        column_types: &[u8],
        flags: &[u16],
    ) -> Result<Row, ParserError> {
        if column_types.len() != columns.len() {
            return Err(ParserError::new(String::from("Column types mismatch")));
        }

        if flags.len() != columns.len() {
            return Err(ParserError::new(String::from("Column flags mismatch")));
        }

        let mut offset = 0;

        let mut values = Vec::with_capacity(columns.len());

        for index in 0..columns.len() {
            let raw_value = Self::read_lenenc_string_or_null(payload, &mut offset)?;

            let value = Self::decode_value(
                raw_value,
                &columns[index].value_type,
                column_types[index],
                flags[index],
            )?;

            values.push(value);
        }

        Ok(Row { content: values })
    }

    fn decode_value(
        raw: Option<&[u8]>,
        value_type: &ValueType,
        column_type: u8,
        flags: u16,
    ) -> Result<Value, ParserError> {
        let Some(raw) = raw else {
            return Ok(Value::Null);
        };

        match value_type {
            ValueType::Int => Self::decode_integer(raw, column_type, flags),

            ValueType::UInt => Self::decode_unsigned_integer(raw, column_type),

            ValueType::Float => Self::decode_float(raw),

            ValueType::Bool => Self::decode_bool(raw),

            ValueType::String => Self::decode_string(raw),

            ValueType::Bytes => Ok(Value::Bytes(raw.to_vec())),
        }
    }

    fn decode_integer(raw: &[u8], column_type: u8, flags: u16) -> Result<Value, ParserError> {
        if flags & UNSIGNED_FLAG != 0 {
            return Self::decode_unsigned_integer(raw, column_type);
        }

        let value = std::str::from_utf8(raw)
            .map_err(|_| ParserError::new(String::from("Invalid MySQL integer")))?
            .parse::<i64>()
            .map_err(|_| ParserError::new(String::from("Invalid MySQL integer")))?;

        Ok(Value::Int(value))
    }

    fn decode_unsigned_integer(raw: &[u8], _column_type: u8) -> Result<Value, ParserError> {
        let value = std::str::from_utf8(raw)
            .map_err(|_| ParserError::new(String::from("Invalid MySQL unsigned integer")))?
            .parse::<u64>()
            .map_err(|_| ParserError::new(String::from("Invalid MySQL unsigned integer")))?;

        Ok(Value::UInt(value))
    }

    fn decode_float(raw: &[u8]) -> Result<Value, ParserError> {
        let value = std::str::from_utf8(raw)
            .map_err(|_| ParserError::new(String::from("Invalid MySQL float")))?
            .parse::<f64>()
            .map_err(|_| ParserError::new(String::from("Invalid MySQL float")))?;

        Ok(Value::Float(value))
    }

    fn decode_bool(raw: &[u8]) -> Result<Value, ParserError> {
        match raw {
            b"0" => Ok(Value::Bool(false)),
            b"1" => Ok(Value::Bool(true)),

            _ => {
                let value = std::str::from_utf8(raw)
                    .map_err(|_| ParserError::new(String::from("Invalid MySQL boolean")))?
                    .parse::<u8>()
                    .map_err(|_| ParserError::new(String::from("Invalid MySQL boolean")))?;

                match value {
                    0 => Ok(Value::Bool(false)),
                    1 => Ok(Value::Bool(true)),
                    _ => Err(ParserError::new(String::from("Invalid MySQL boolean"))),
                }
            }
        }
    }

    fn decode_string(raw: &[u8]) -> Result<Value, ParserError> {
        let value = std::str::from_utf8(raw)
            .map_err(|_| ParserError::new(String::from("Invalid UTF-8 MySQL string")))?
            .to_string();

        Ok(Value::String(value))
    }

    fn value_type_from_mysql_type(column_type: u8) -> Result<ValueType, ParserError> {
        match column_type {
            MYSQL_TYPE_TINY | MYSQL_TYPE_SHORT | MYSQL_TYPE_LONG | MYSQL_TYPE_INT24
            | MYSQL_TYPE_LONGLONG => Ok(ValueType::Int),

            MYSQL_TYPE_FLOAT | MYSQL_TYPE_DOUBLE | MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL => {
                Ok(ValueType::Float)
            }

            MYSQL_TYPE_BIT => Ok(ValueType::Bool),

            MYSQL_TYPE_VARCHAR
            | MYSQL_TYPE_VAR_STRING
            | MYSQL_TYPE_STRING
            | MYSQL_TYPE_DATE
            | MYSQL_TYPE_TIME
            | MYSQL_TYPE_DATETIME
            | MYSQL_TYPE_TIMESTAMP
            | MYSQL_TYPE_YEAR
            | MYSQL_TYPE_JSON => Ok(ValueType::String),

            MYSQL_TYPE_BLOB => Ok(ValueType::Bytes),

            MYSQL_TYPE_NULL => Ok(ValueType::String),

            _ => Err(ParserError::new(format!(
                "Unsupported MySQL column type: {}",
                column_type
            ))),
        }
    }

    fn read_lenenc_int(payload: &[u8], offset: &mut usize) -> Result<Option<u64>, ParserError> {
        let first = Self::read_u8(payload, offset)?;

        match first {
            0x00..=0xfa => Ok(Some(first as u64)),

            0xfb => Ok(None),

            0xfc => {
                let value = Self::read_u16_le(payload, offset)?;

                Ok(Some(value as u64))
            }

            0xfd => {
                let bytes = Self::read_bytes(payload, offset, 3)?;

                let value =
                    (bytes[0] as u64) | ((bytes[1] as u64) << 8) | ((bytes[2] as u64) << 16);

                Ok(Some(value))
            }

            0xfe => {
                let value = Self::read_u64_le(payload, offset)?;

                Ok(Some(value))
            }

            0xff => Err(ParserError::new(String::from(
                "Invalid MySQL length-encoded integer",
            ))),
        }
    }

    fn read_lenenc_string(payload: &[u8], offset: &mut usize) -> Result<String, ParserError> {
        let raw = Self::read_lenenc_bytes(payload, offset)?;

        let value = std::str::from_utf8(raw)
            .map_err(|_| ParserError::new(String::from("Invalid UTF-8 MySQL string")))?
            .to_string();

        Ok(value)
    }

    fn read_lenenc_string_or_null<'a>(
        payload: &'a [u8],
        offset: &mut usize,
    ) -> Result<Option<&'a [u8]>, ParserError> {
        let length = Self::read_lenenc_int(payload, offset)?;

        let Some(length) = length else {
            return Ok(None);
        };

        let length = usize::try_from(length)
            .map_err(|_| ParserError::new(String::from("MySQL value length exceeds usize")))?;

        let value = Self::read_bytes(payload, offset, length)?;

        Ok(Some(value))
    }

    fn read_lenenc_bytes<'a>(
        payload: &'a [u8],
        offset: &mut usize,
    ) -> Result<&'a [u8], ParserError> {
        let length = Self::read_lenenc_int(payload, offset)?;

        let Some(length) = length else {
            return Err(ParserError::new(String::from(
                "Unexpected NULL length-encoded string",
            )));
        };

        let length = usize::try_from(length)
            .map_err(|_| ParserError::new(String::from("MySQL string length exceeds usize")))?;

        Self::read_bytes(payload, offset, length)
    }

    fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, ParserError> {
        if *offset >= payload.len() {
            return Err(ParserError::new(String::from(
                "Unexpected end of MySQL payload",
            )));
        }

        let value = payload[*offset];

        *offset += 1;

        Ok(value)
    }

    fn read_u16_le(payload: &[u8], offset: &mut usize) -> Result<u16, ParserError> {
        let bytes = Self::read_bytes(payload, offset, 2)?;

        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(payload: &[u8], offset: &mut usize) -> Result<u32, ParserError> {
        let bytes = Self::read_bytes(payload, offset, 4)?;

        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64_le(payload: &[u8], offset: &mut usize) -> Result<u64, ParserError> {
        let bytes = Self::read_bytes(payload, offset, 8)?;

        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_bytes<'a>(
        payload: &'a [u8],
        offset: &mut usize,
        length: usize,
    ) -> Result<&'a [u8], ParserError> {
        if *offset + length > payload.len() {
            return Err(ParserError::new(String::from(
                "Unexpected end of MySQL payload",
            )));
        }

        let value = &payload[*offset..*offset + length];

        *offset += length;

        Ok(value)
    }
}

pub struct ParserError {
    message: String,
}

impl ParserError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl From<ParserError> for DbError {
    fn from(error: ParserError) -> Self {
        DbError::new(error.message)
    }
}

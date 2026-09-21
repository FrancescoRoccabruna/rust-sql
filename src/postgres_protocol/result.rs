use crate::{
    DbError,
    table::{Column, Row, Value, ValueType},
};

type RowDescription = (Vec<Column>, Vec<u32>, Vec<i16>);

pub struct ResultParser {}

impl ResultParser {
    pub fn parse_row_description(payload: &[u8]) -> Result<RowDescription, ParserError> {
        if payload.len() < 2 {
            return Err(ParserError::new(String::from(
                "Invalid RowDescription payload",
            )));
        }

        let mut offset = 0;

        let number_of_fields = i16::from_be_bytes([payload[offset], payload[offset + 1]]);

        offset += 2;

        if number_of_fields < 0 {
            return Err(ParserError::new(String::from("Invalid number of fields")));
        }

        let mut columns = Vec::new();
        let mut type_oids = Vec::new();
        let mut format_codes = Vec::new();

        for _ in 0..number_of_fields {
            let name = Self::read_string(payload, &mut offset)?;

            let _table_oid = Self::read_u32(payload, &mut offset)?;
            let _attribute_number = Self::read_i16(payload, &mut offset)?;
            let type_oid = Self::read_u32(payload, &mut offset)?;
            let _type_size = Self::read_i16(payload, &mut offset)?;
            let _type_modifier = Self::read_i32(payload, &mut offset)?;
            let format_code = Self::read_i16(payload, &mut offset)?;

            let value_type = Self::value_type_from_oid(type_oid)?;

            let column = Column { name, value_type };

            columns.push(column);
            type_oids.push(type_oid);
            format_codes.push(format_code);
        }

        Ok((columns, type_oids, format_codes))
    }

    fn read_string(payload: &[u8], offset: &mut usize) -> Result<String, ParserError> {
        let start = *offset;

        while *offset < payload.len() {
            if payload[*offset] == 0 {
                let value = std::str::from_utf8(&payload[start..*offset])
                    .map_err(|_| ParserError::new(String::from("Invalid UTF-8 string")))?
                    .to_string();

                *offset += 1;

                return Ok(value);
            }

            *offset += 1;
        }

        Err(ParserError::new(String::from("Missing null terminator")))
    }

    fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, ParserError> {
        if *offset + 4 > payload.len() {
            return Err(ParserError::new(String::from("Unexpected end of payload")));
        }

        let value = u32::from_be_bytes([
            payload[*offset],
            payload[*offset + 1],
            payload[*offset + 2],
            payload[*offset + 3],
        ]);

        *offset += 4;

        Ok(value)
    }

    fn read_i16(payload: &[u8], offset: &mut usize) -> Result<i16, ParserError> {
        if *offset + 2 > payload.len() {
            return Err(ParserError::new(String::from("Unexpected end of payload")));
        }

        let value = i16::from_be_bytes([payload[*offset], payload[*offset + 1]]);

        *offset += 2;

        Ok(value)
    }

    fn read_i32(payload: &[u8], offset: &mut usize) -> Result<i32, ParserError> {
        if *offset + 4 > payload.len() {
            return Err(ParserError::new(String::from("Unexpected end of payload")));
        }

        let value = i32::from_be_bytes([
            payload[*offset],
            payload[*offset + 1],
            payload[*offset + 2],
            payload[*offset + 3],
        ]);

        *offset += 4;

        Ok(value)
    }

    fn value_type_from_oid(type_oid: u32) -> Result<ValueType, ParserError> {
        match type_oid {
            16 => Ok(ValueType::Bool),     // bool
            20 => Ok(ValueType::Int),      // int8
            21 => Ok(ValueType::Int),      // int2
            23 => Ok(ValueType::Int),      // int4
            700 => Ok(ValueType::Float),   // float4
            701 => Ok(ValueType::Float),   // float8
            25 => Ok(ValueType::String),   // text
            1043 => Ok(ValueType::String), // varchar
            1700 => Ok(ValueType::Float),  // numeric

            _ => Err(ParserError::new(format!(
                "Unsupported PostgreSQL type OID: {}",
                type_oid
            ))),
        }
    }

    fn read_value<'a>(
        payload: &'a [u8],
        offset: &mut usize,
    ) -> Result<Option<&'a [u8]>, ParserError> {
        let length = Self::read_i32(payload, offset)?;

        if length == -1 {
            return Ok(None);
        }

        if length < 0 {
            return Err(ParserError::new(String::from(
                "Invalid DataRow value length",
            )));
        }

        let length = length as usize;

        if *offset + length > payload.len() {
            return Err(ParserError::new(String::from(
                "Unexpected end of DataRow payload",
            )));
        }

        let value = &payload[*offset..*offset + length];

        *offset += length;

        Ok(Some(value))
    }

    pub fn parse_data_row(
        payload: &[u8],
        columns: &[Column],
        type_oids: &[u32],
        format_codes: &[i16],
    ) -> Result<Row, ParserError> {
        if payload.len() < 2 {
            return Err(ParserError::new(String::from("Invalid DataRow payload")));
        }

        let mut offset = 0;

        let number_of_columns = Self::read_i16(payload, &mut offset)?;

        if number_of_columns < 0 {
            return Err(ParserError::new(String::from("Invalid number of columns")));
        }

        if number_of_columns as usize != columns.len() {
            return Err(ParserError::new(String::from("DataRow columns mismatch")));
        }

        if format_codes.len() != columns.len() {
            return Err(ParserError::new(String::from("Format codes mismatch")));
        }

        if type_oids.len() != columns.len() {
            return Err(ParserError::new(String::from("Type OIDs mismatch")));
        }

        let mut values = Vec::with_capacity(columns.len());

        for index in 0..columns.len() {
            let raw_value = Self::read_value(payload, &mut offset)?;

            let value = Self::decode_value(
                raw_value,
                &columns[index].value_type,
                type_oids[index],
                format_codes[index],
            )?;

            values.push(value);
        }

        Ok(Row { content: values })
    }

    fn decode_value(
        raw: Option<&[u8]>,
        value_type: &ValueType,
        type_oid: u32,
        format_code: i16,
    ) -> Result<Value, ParserError> {
        let Some(raw) = raw else {
            return Ok(Value::Null);
        };

        match format_code {
            0 => Self::decode_text_value(raw, value_type),
            1 => Self::decode_binary_value(raw, value_type, type_oid),
            _ => Err(ParserError::new(String::from(
                "Unsupported PostgreSQL format code",
            ))),
        }
    }

    fn decode_text_value(raw: &[u8], value_type: &ValueType) -> Result<Value, ParserError> {
        match value_type {
            ValueType::Bool => match raw {
                b"t" => Ok(Value::Bool(true)),
                b"f" => Ok(Value::Bool(false)),
                _ => Err(ParserError::new(String::from("Invalid PostgreSQL boolean"))),
            },

            ValueType::Int => {
                let value = std::str::from_utf8(raw)
                    .map_err(|_| ParserError::new(String::from("Invalid UTF-8 integer")))?
                    .parse::<i64>()
                    .map_err(|_| ParserError::new(String::from("Invalid PostgreSQL integer")))?;

                Ok(Value::Int(value))
            }

            ValueType::Float => {
                let value = std::str::from_utf8(raw)
                    .map_err(|_| ParserError::new(String::from("Invalid UTF-8 float")))?
                    .parse::<f64>()
                    .map_err(|_| ParserError::new(String::from("Invalid PostgreSQL float")))?;

                Ok(Value::Float(value))
            }

            ValueType::String => {
                let value = std::str::from_utf8(raw)
                    .map_err(|_| ParserError::new(String::from("Invalid UTF-8 string")))?
                    .to_string();

                Ok(Value::String(value))
            }

            _ => Err(ParserError::new(String::from(
                "Unsupported PostgreSQL text type",
            ))),
        }
    }

    fn decode_binary_value(
        raw: &[u8],
        value_type: &ValueType,
        type_oid: u32,
    ) -> Result<Value, ParserError> {
        match value_type {
            ValueType::Bool => {
                if raw.len() != 1 {
                    return Err(ParserError::new(String::from("Invalid PostgreSQL boolean")));
                }

                match raw[0] {
                    0 => Ok(Value::Bool(false)),
                    1 => Ok(Value::Bool(true)),
                    _ => Err(ParserError::new(String::from("Invalid PostgreSQL boolean"))),
                }
            }

            ValueType::Int => match type_oid {
                21 => {
                    if raw.len() != 2 {
                        return Err(ParserError::new(String::from("Invalid int2 value")));
                    }

                    let value = i16::from_be_bytes([raw[0], raw[1]]);

                    Ok(Value::Int(value as i64))
                }

                23 => {
                    if raw.len() != 4 {
                        return Err(ParserError::new(String::from("Invalid int4 value")));
                    }

                    let value = i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);

                    Ok(Value::Int(value as i64))
                }

                20 => {
                    if raw.len() != 8 {
                        return Err(ParserError::new(String::from("Invalid int8 value")));
                    }

                    let value = i64::from_be_bytes([
                        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                    ]);

                    Ok(Value::Int(value))
                }

                _ => Err(ParserError::new(format!(
                    "Unsupported PostgreSQL integer OID: {}",
                    type_oid
                ))),
            },

            ValueType::Float => match type_oid {
                700 => {
                    if raw.len() != 4 {
                        return Err(ParserError::new(String::from("Invalid float4 value")));
                    }

                    let bits = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);

                    Ok(Value::Float(f32::from_bits(bits) as f64))
                }

                701 => {
                    if raw.len() != 8 {
                        return Err(ParserError::new(String::from("Invalid float8 value")));
                    }

                    let bits = u64::from_be_bytes([
                        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                    ]);

                    Ok(Value::Float(f64::from_bits(bits)))
                }

                _ => Err(ParserError::new(format!(
                    "Unsupported PostgreSQL float OID: {}",
                    type_oid
                ))),
            },

            ValueType::String => {
                let value = std::str::from_utf8(raw)
                    .map_err(|_| ParserError::new(String::from("Invalid UTF-8 string")))?
                    .to_string();

                Ok(Value::String(value))
            }

            _ => Err(ParserError::new(String::from(
                "Unsupported PostgreSQL binary type",
            ))),
        }
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

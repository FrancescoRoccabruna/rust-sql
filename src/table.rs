use std::fmt;

/// Describes a column returned by a query.
#[derive(Debug)]
pub struct ResultColumn {
    /// ResultColumn name.
    pub name: String,

    /// Type of values stored in the column.
    pub value_type: ValueType,
}

impl fmt::Display for ResultColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A row returned by a database query.
#[derive(Debug)]
pub struct ResultRow {
    pub(crate) content: Vec<Value>,
}

impl ResultRow {
    /// Returns the number of values in the row.
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// Returns the values contained in the row.
    pub fn values(&self) -> &[Value] {
        &self.content
    }
}

impl fmt::Display for ResultRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, value) in self.content.iter().enumerate() {
            if i > 0 {
                write!(f, " | ")?;
            }

            write!(f, "{value}")?;
        }

        Ok(())
    }
}

/// A value returned by a database query.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL value.
    Null,

    /// Signed integer.
    Int(i64),

    /// Unsigned integer.
    UInt(u64),

    /// Floating-point number.
    Float(f64),

    /// Text value.
    String(String),

    /// Binary data.
    Bytes(Vec<u8>),

    /// Boolean value.
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Int(value) => write!(f, "{value}"),
            Value::UInt(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::String(value) => write!(f, "{value}"),
            Value::Bytes(value) => write!(f, "{value:?}"),
            Value::Bool(value) => write!(f, "{value}"),
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Int(value as i64)
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Value::Int(value as i64)
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Value::Int(value as i64)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::UInt(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Value::UInt(value as u64)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value::UInt(value as u64)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Value::UInt(value as u64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Float(value as f64)
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::UInt(value as u64)
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Value::Int(value as i64)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Value::Bytes(value)
    }
}

impl From<&[u8]> for Value {
    fn from(value: &[u8]) -> Self {
        Value::Bytes(value.to_vec())
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Value::Null,
        }
    }
}

/// Describes the type of a database value.
#[derive(Debug, Clone, Copy)]
pub enum ValueType {
    /// Signed integer.
    Int,

    /// Unsigned integer.
    UInt,

    /// Floating-point number.
    Float,

    /// Text value.
    String,

    /// Binary data.
    Bytes,

    /// Boolean value.
    Bool,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Int => write!(f, "Int"),
            ValueType::UInt => write!(f, "UInt"),
            ValueType::Float => write!(f, "Float"),
            ValueType::String => write!(f, "String"),
            ValueType::Bytes => write!(f, "Bytes"),
            ValueType::Bool => write!(f, "Bool"),
        }
    }
}

/// Tabular representation of a query result.
#[derive(Debug)]
pub struct Dataframe {
    columns: Vec<ResultColumn>,
    rows: Vec<ResultRow>,
}

impl Dataframe {
    /// Creates a dataframe from columns and rows.
    ///
    /// Returns an error if a row contains a different number of values
    /// than the number of columns.
    pub fn new(columns: Vec<ResultColumn>, rows: Vec<ResultRow>) -> Result<Self, DfError> {
        for row in &rows {
            if row.size() != columns.len() {
                return Err(DfError::new(String::from("Mismatch rows size")));
            }
        }

        Ok(Self { columns, rows })
    }
}

impl fmt::Display for Dataframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, column) in self.columns.iter().enumerate() {
            if i > 0 {
                write!(f, " | ")?;
            }

            write!(f, "{column}")?;
        }

        writeln!(f)?;

        for row in &self.rows {
            writeln!(f, "{row}")?;
        }

        Ok(())
    }
}

/// Error returned when a dataframe cannot be constructed.
#[derive(Debug)]
pub struct DfError {
    pub message: String,
}

impl DfError {
    /// Creates a new dataframe error.
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for DfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DfError {}

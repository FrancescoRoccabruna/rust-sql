use std::fmt;

/// Describes a column returned by a query.
#[derive(Debug)]
pub struct Column {
    /// Column name.
    pub name: String,

    /// Type of values stored in the column.
    pub value_type: ValueType,
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A row returned by a database query.
#[derive(Debug)]
pub struct Row {
    pub(crate) content: Vec<Value>,
}

impl Row {
    /// Returns the number of values in the row.
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// Returns the values contained in the row.
    pub fn values(&self) -> &[Value] {
        &self.content
    }
}

impl fmt::Display for Row {
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
#[derive(Debug)]
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

/// Describes the type of a database value.
#[derive(Debug)]
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
    columns: Vec<Column>,
    rows: Vec<Row>,
}

impl Dataframe {
    /// Creates a dataframe from columns and rows.
    ///
    /// Returns an error if a row contains a different number of values
    /// than the number of columns.
    pub fn new(columns: Vec<Column>, rows: Vec<Row>) -> Result<Self, DfError> {
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

use std::fmt;


#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub value_type: ValueType,
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }

}



#[derive(Debug)]
pub struct Row {
    pub(crate) content: Vec<Value>,
}

impl Row {
    pub fn size(&self) -> usize {
        self.content.len()
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


#[derive(Debug)]
pub enum Value {
    Null,
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
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


#[derive(Debug)]
pub enum ValueType {
    Int,
    UInt,
    Float,
    String,
    Bytes,
    Bool,
}

impl fmt::Display for ValueType{
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



#[derive(Debug)]
pub struct Dataframe {
    columns: Vec<Column>,
    rows: Vec<Row>,
}


impl Dataframe {
    pub fn new(
        columns: Vec<Column>,
        rows: Vec<Row>
    ) -> Result<Self, DfError> {
        for row in &rows{
            if row.size() != columns.len() {
                return Err(DfError::new(
                    String::from("Mismatch rows size")
                ));
            }
        }

        Ok(Self {
            columns,
            rows,
        })
    }
}

impl fmt::Display for Dataframe{
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

#[derive(Debug)]
pub struct DfError {
    pub message: String
}


impl DfError {
    pub fn new(message: String) -> Self {
        Self {
            message,
        }
    }
}
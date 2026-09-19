
#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub value_type: ValueType,
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
#[derive(Debug)]
pub enum ValueType {
    Int,
    UInt,
    Float,
    String,
    Bytes,
    Bool,
}




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
use crate::table::{Column, Dataframe, DfError, Row};


pub struct Query {
    sql: String,
}


impl Query {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
        }
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }
}

pub struct QueryResult {
    columns: Vec<Column>,
    rows: Vec<Row>,
}


impl  QueryResult {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    //pub fn add(&mut self, message: ServerMessage){
    //    self.messages.push(message);
    //}

    pub(crate) fn set_columns(&mut self, columns: Vec<Column>){
        self.columns = columns;
    }

    pub(crate) fn add_row(&mut self, row: Row){
        self.rows.push(row);
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn dataframe(self) -> Result<Dataframe, DfError> {
        Dataframe::new(self.columns, self.rows)
    }
}
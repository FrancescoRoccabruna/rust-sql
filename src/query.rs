use crate::table::{Column, Dataframe, DfError, Row};

/// A SQL query to be executed by a database connection.
pub struct Query {
    sql: String,
}

impl Query {
    /// Creates a query from a SQL string.
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
        }
    }

    /// Returns the SQL statement.
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

pub struct QueryResult {
    columns: Vec<Column>,
    rows: Vec<Row>,
}

/// Result returned after executing a SQL query.
impl QueryResult {
    /// Creates an empty query result.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn set_columns(&mut self, columns: Vec<Column>) {
        self.columns = columns;
    }

    pub(crate) fn add_row(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// Returns the rows returned by the query.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Returns the columns returned by the query.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Converts the result into a [`Dataframe`].
    pub fn dataframe(self) -> Result<Dataframe, DfError> {
        Dataframe::new(self.columns, self.rows)
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}

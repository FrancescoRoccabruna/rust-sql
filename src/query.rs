use crate::{
    Connection, DbError, Session, Value, orm::query::SessionExecutableQuery, table::{Dataframe, DfError, ResultColumn, ResultRow},
};

/// A SQL query to be executed by a database connection.
pub struct Query {
    sql: String,
    params: Vec<Value>,
}

impl Query {
    /// Creates a query from a SQL string.
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            params: Vec::new(),
        }
    }

    /// Creates a parameterized SQL query.
    pub fn with_params(sql: &str, params: Vec<Value>) -> Self {
        Self {
            sql: sql.to_string(),
            params,
        }
    }

    /// Returns the SQL statement.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Returns the query parameters.
    pub(crate) fn params(&self) -> &[Value] {
        &self.params
    }
}

pub struct QueryResult {
    columns: Vec<ResultColumn>,
    rows: Vec<ResultRow>,
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

    pub(crate) fn set_columns(&mut self, columns: Vec<ResultColumn>) {
        self.columns = columns;
    }

    pub(crate) fn add_row(&mut self, row: ResultRow) {
        self.rows.push(row);
    }

    /// Returns the rows returned by the query.
    pub fn rows(&self) -> &[ResultRow] {
        &self.rows
    }

    /// Returns the columns returned by the query.
    pub fn columns(&self) -> &[ResultColumn] {
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

pub trait ExecutableQuery {
    type Output;

    fn execute(&self, connection: &mut Connection) -> Result<Self::Output, DbError>;
}

impl ExecutableQuery for Query {
    type Output = QueryResult;

    fn execute(&self, connection: &mut Connection) -> Result<Self::Output, DbError> {
        connection.execute_raw(self)
    }
}


impl SessionExecutableQuery for Query {
    type Output = QueryResult;

    fn execute_in_session(
        &self,
        session: &mut Session,
    ) -> Result<Self::Output, DbError> {
        session.execute_query(self)
    }
}

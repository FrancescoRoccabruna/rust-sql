use crate::{
    DbError,
    query::{Query, QueryResult},
};

pub(crate) mod mysql;
pub(crate) mod postgres;

pub(crate) trait Backend {
    fn open(&mut self, username: &str, password: &str, db_name: &str) -> Result<(), DbError>;

    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError>;

    fn start_transaction(&mut self) -> Result<(), DbError>;

    fn commit_transaction(&mut self) -> Result<(), DbError>;

    fn rollback_transaction(&mut self) -> Result<(), DbError>;
}

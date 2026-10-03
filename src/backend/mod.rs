use crate::{
    DbError, orm::{TableDefinition, query::{DeleteQueryRequest, InsertQueryRequest, SelectQueryRequest, UpdateQueryRequest}}, query::{Query, QueryResult},
};

pub(crate) mod mysql;
pub(crate) mod postgres;

pub(crate) trait Backend {
    fn open(&mut self, username: &str, password: &str, db_name: &str) -> Result<(), DbError>;

    fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        if query.params().is_empty() {
            self.exec_simple(query)
        } else {
            self.exec_prepared(query)
        }
    }

    fn exec_simple(&mut self, query: &Query) -> Result<QueryResult, DbError>;

    fn exec_prepared(&mut self, query: &Query) -> Result<QueryResult, DbError>;

    fn exec_orm(&mut self, query: &dyn SelectQueryRequest) -> Result<QueryResult, DbError>;

    fn start_transaction(&mut self) -> Result<(), DbError>;

    fn commit_transaction(&mut self) -> Result<(), DbError>;

    fn rollback_transaction(&mut self) -> Result<(), DbError>;

    fn create_table(&mut self, table: &TableDefinition) -> Result<(), DbError>;

    fn exec_insert(
        &mut self,
        query: &dyn InsertQueryRequest,
    ) -> Result<(), DbError>;


    fn exec_update(
        &mut self,
        query: &dyn UpdateQueryRequest,
    ) -> Result<(), DbError>;

    fn exec_delete(
        &mut self,
        query: &dyn DeleteQueryRequest,
    ) -> Result<(), DbError>;
}

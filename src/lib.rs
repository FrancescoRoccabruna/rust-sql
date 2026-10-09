extern crate self as rust_sql;

mod backend;
mod config;
mod connection;
mod mysql_protocol;
pub mod orm;
mod postgres_protocol;
mod query;
mod table;

pub use config::{DatabaseConfig, DatabaseKind};

pub use connection::{
    Connection, ConnectionPool, DbError, PooledConnection, Session, SessionMaker,
};

pub use query::{ExecutableQuery, Query, QueryResult};

pub use table::{Dataframe, DfError, ResultColumn, ResultRow, Value, ValueType};

pub use orm::Entity;

pub use rust_sql_derive::Table;

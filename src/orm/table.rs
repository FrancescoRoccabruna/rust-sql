use crate::{DbError, ResultRow, Value, orm::column::ColumnRef};

pub trait Table: Sized {
    fn table_name() -> &'static str;
    fn fields() -> Vec<ColumnRef>;
    fn values(&self) -> Vec<(&'static str, Value)>;

    fn from_row(row: &ResultRow) -> Result<Self, DbError>;
}

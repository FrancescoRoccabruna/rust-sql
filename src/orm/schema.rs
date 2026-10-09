use crate::{
    Connection, DatabaseConfig, DbError,
    orm::{ColumnRef, Table, column::ForeignKeyRef},
};

pub struct Schema {
    connection: Connection,
    tables: Vec<TableDefinition>,
}

impl Schema {
    pub fn new(config: &DatabaseConfig) -> Result<Self, DbError> {
        Ok(Self {
            connection: config.connect()?,
            tables: Vec::new(),
        })
    }

    pub fn table<T: Table>(mut self) -> Self {
        self.tables.push(TableDefinition {
            name: T::table_name(),
            fields: T::fields(),
            foreign_keys: T::foreign_keys(),
        });

        self
    }

    pub fn create_all(mut self) -> Result<(), DbError> {
        for table in &self.tables {
            self.connection.create_table(table)?;
        }

        Ok(())
    }
}

pub(crate) struct TableDefinition {
    pub name: &'static str,
    pub fields: Vec<ColumnRef>,
    pub foreign_keys: Vec<ForeignKeyRef>,
}

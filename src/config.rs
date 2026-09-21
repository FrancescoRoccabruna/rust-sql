use crate::connection::{Connection, DbError};

/// Configuration used to establish a database connection.
pub struct DatabaseConfig {
    kind: DatabaseKind,
    host: String,
    port: u16,
    username: String,
    password: String,
    db_name: String,
}

impl DatabaseConfig {
    /// Creates a new database configuration.
    pub fn new(
        kind: DatabaseKind,
        host: String,
        port: u16,
        username: String,
        password: String,
        db_name: String,
    ) -> Self {
        Self {
            kind,
            host,
            port,
            username,
            password,
            db_name,
        }
    }

    /// Opens a connection using this configuration.
    pub fn connect(&self) -> Result<Connection, DbError> {
        if self.host.is_empty() {
            return Err(DbError::new(String::from("Host is empty")));
        }
        let mut connection = Connection::new(self.host.clone(), self.port, self.kind.clone());

        connection.open(&self.username, &self.password, &self.db_name)?;

        Ok(connection)
    }
}

/// Supported database engines.
#[derive(Clone)]
pub enum DatabaseKind {
    Postgres,
    MySql,
}

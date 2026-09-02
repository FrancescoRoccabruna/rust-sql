use crate::{
    connection::{Connection, DbError},
};



pub struct DatabaseConfig {
    kind: DatabaseKind,
    host: String,
    port: u16,
    username: String,
    password: String,
    db_name: String,
}


impl DatabaseConfig {
    pub fn new(
        kind: DatabaseKind,
        host: String,
        port: u16, username: String,
        password: String,
        db_name: String
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

    pub fn connect(self) -> Result<Connection, DbError> {
        if self.host.is_empty(){
            return Err(DbError::new(String::from("Host is empty")));
        }
        let mut connection = Connection::new(self.host, self.port, self.kind);

        connection.open(
            &self.username,
            &self.password,
            &self.db_name
        )?;

        Ok(connection)
    }

}


pub enum DatabaseKind {
    Postgres,
    MySql,
}
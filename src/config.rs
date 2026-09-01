use crate::{
    connection::{Connection, DbError},
};



pub struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    db_name: String,
}


impl DatabaseConfig {
    pub fn new(
        host: String,
        port: u16, username: String,
        password: String,
        db_name: String
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            db_name,
        }
    }

    pub fn connect(&self) -> Result<Connection, DbError> {
        if self.host.is_empty(){
            return Err(DbError::new(String::from("Host is empty")));
        }
        let mut connection = Connection::new(self.host.clone(), self.port);

        connection.open(
            &self.username,
            &self.password,
            &self.db_name
        )?;

        Ok(connection)
    }

}
mod config;
mod connection;
mod postgres_protocol;
mod query;

pub use config::DatabaseConfig;
pub use connection::{Connection, DbError};



#[cfg(test)]
mod tests {
    use crate::{DatabaseConfig, config::DatabaseKind, query::Query};


    #[test]
    fn connection_test() {
        let config = DatabaseConfig::new(
            DatabaseKind::Postgres,
            String::from("localhost"),
            5432,
            String::from("postgres"),
            String::from("password"),
            String::from("testdb"),
        );

        let connection = config.connect();

        assert!(connection.is_ok());

        let mut connection = connection.unwrap();

        let mut query = Query::new("SELECT 1;");


        connection.exec(&query);

        query = Query::new("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT);");

        connection.exec(&query);


        
    }
}


















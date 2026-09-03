mod config;
mod connection;
mod postgres_protocol;
mod mysql_protocol;
mod query;
mod backend;

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


        let config = DatabaseConfig::new(
            DatabaseKind::MySql,
            String::from("localhost"),
            3306,
            String::from("mysql"),
            String::from("password"),
            String::from("testdb"),
        );

        let connection = config.connect()
            .expect("MySQL connection failed");

        //assert!(connection.is_ok());


        
    }
}


















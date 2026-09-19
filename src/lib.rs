mod config;
mod connection;
mod postgres_protocol;
mod mysql_protocol;
mod query;
mod backend;
mod table;

pub use config::DatabaseConfig;
pub use connection::{Connection, DbError};



#[cfg(test)]
mod tests {
    use crate::{DatabaseConfig, config::DatabaseKind, query::Query, table::Value};


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


        connection.exec(&query).unwrap();

        query = Query::new("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT);");

        connection.exec(&query).unwrap();


        query = Query::new("SELECT 1;");

        let result = connection.exec(&query);

        assert!(result.is_ok());

        let result = result.unwrap();

        assert_eq!(result.columns().len(), 1);
        assert_eq!(result.rows().len(), 1);

        let row = &result.rows()[0];

        assert_eq!(row.size(), 1);

        match &row.content[0] {
            Value::Int(value) => assert_eq!(*value, 1),
            value => panic!("Expected Value::Int(1), got {:?}", value),
        }


        query = Query::new("SELECT 1, 'hello', true, 3.14, NULL;");

        let result = connection.exec(&query);

        assert!(result.is_ok());

        let result = result.unwrap();

        assert_eq!(result.columns().len(), 5);
        assert_eq!(result.rows().len(), 1);

        let row = &result.rows()[0];

        assert_eq!(row.size(), 5);

        match &row.content[0] {
            Value::Int(value) => assert_eq!(*value, 1),
            value => panic!("Expected Value::Int(1), got {:?}", value),
        }

        match &row.content[1] {
            Value::String(value) => assert_eq!(*value, "hello"),
            value => panic!("Expected Value::String('hello'), got {:?}", value),
        }
        match &row.content[2] {
            Value::Bool(value) => assert_eq!(*value, true),
            value => panic!("Expected Value::Bool(true), got {:?}", value),
        }
        match &row.content[3] {
            Value::Float(value) => assert_eq!(*value, 3.14),
            value => panic!("Expected Value::Float(3.14), got {:?}", value),
        }
        match &row.content[4] {
            Value::Null => (),
            value => panic!("Expected Value::Null, got {:?}", value),
        }

        query = Query::new("DELETE FROM users;");

        connection.exec(&query).unwrap();


        query = Query::new("INSERT INTO users(id, name) VALUES (1, 'mario'), (2, 'giovanni'), (3, 'sandro');");

        connection.exec(&query).unwrap();

        query = Query::new("SELECT * FROM users;");

        let result = connection.exec(&query);

        assert!(result.is_ok());

        let result = result.unwrap();

        assert_eq!(result.columns().len(), 2);
        assert_eq!(result.rows().len(), 3);

        let row = &result.rows()[0];

        assert_eq!(row.size(), 2);

        match &row.content[0] {
            Value::Int(value) => assert_eq!(*value, 1),
            value => panic!("Expected Value::Int(1), got {:?}", value),
        }

        match &row.content[1] {
            Value::String(value) => assert_eq!(*value, "mario"),
            value => panic!("Expected Value::String('mario'), got {:?}", value),
        }


        


        let config = DatabaseConfig::new(
            DatabaseKind::MySql,
            String::from("localhost"),
            3307,
            String::from("mysql"),
            String::from("password"),
            String::from("testdb"),
        );


        let connection = config.connect();


        assert!(connection.is_ok());

        let mut connection = connection.unwrap();

        query = Query::new("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name VARCHAR(32));");

        connection.exec(&query).unwrap();

    }
}


















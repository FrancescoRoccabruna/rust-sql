mod config;
mod connection;
mod protocol;

pub use config::DatabaseConfig;
pub use connection::{Connection, DbError};



#[cfg(test)]
mod tests {
    use crate::DatabaseConfig;


    #[test]
    fn connection_test() {
        let config = DatabaseConfig::new(
            String::from("localhost"),
            5432,
            String::from("postgres"),
            String::from("password"),
            String::from("testdb"),
        );

        let result = config.connect();

        assert!(result.is_ok());
    }
}


















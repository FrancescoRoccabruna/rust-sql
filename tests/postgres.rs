use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use rust_sql::{Connection, ConnectionPool, DatabaseConfig, DatabaseKind, Query, Value};

fn postgres_config() -> DatabaseConfig {
    DatabaseConfig::new(
        DatabaseKind::Postgres,
        String::from("localhost"),
        5432,
        String::from("postgres"),
        String::from("password"),
        String::from("testdb"),
    )
}

fn unique_table_name(base: &str) -> String {
    format!("{}_{}", base, rand::random::<u64>())
}

fn with_test_table<F>(connection: &mut Connection, base: &str, create_sql: String, test: F)
where
    F: FnOnce(&mut Connection, &str),
{
    let table = unique_table_name(base);

    connection
        .exec(&Query::new(&create_sql.replace("{table}", &table)))
        .unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| {
        test(connection, &table);
    }));

    let cleanup_result = connection.exec(&Query::new(&format!("DROP TABLE {table};")));

    if let Err(error) = cleanup_result {
        panic!("Failed to drop test table {table}: {:?}", error);
    }

    if let Err(payload) = result {
        resume_unwind(payload);
    }
}

#[test]
fn postgres_connection() {
    let config = postgres_config();

    let connection = config.connect();

    assert!(
        connection.is_ok(),
        "PostgreSQL connection failed: {:?}",
        connection.err()
    );
}

#[test]
fn postgres_simple_select() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    let result = connection.exec(&Query::new("SELECT 1;")).unwrap();

    assert_eq!(result.columns().len(), 1);
    assert_eq!(result.rows().len(), 1);

    let row = &result.rows()[0];

    assert_eq!(row.size(), 1);

    match &row.values()[0] {
        Value::Int(value) => {
            assert_eq!(*value, 1);
        }

        value => {
            panic!("Expected Value::Int(1), got {:?}", value);
        }
    }
}

#[allow(clippy::approx_constant)]
#[test]
fn postgres_select_values() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    let result = connection
        .exec(&Query::new("SELECT 1, 'hello', true, 3.14, NULL;"))
        .unwrap();

    assert_eq!(result.columns().len(), 5);
    assert_eq!(result.rows().len(), 1);

    let row = &result.rows()[0];

    assert_eq!(row.size(), 5);

    match &row.values()[0] {
        Value::Int(value) => {
            assert_eq!(*value, 1);
        }

        value => {
            panic!("Expected Value::Int(1), got {:?}", value);
        }
    }

    match &row.values()[1] {
        Value::String(value) => {
            assert_eq!(value, "hello");
        }

        value => {
            panic!("Expected Value::String, got {:?}", value);
        }
    }

    match &row.values()[2] {
        Value::Bool(value) => {
            assert!(*value);
        }

        value => {
            panic!("Expected Value::Bool(true), got {:?}", value);
        }
    }

    match &row.values()[3] {
        Value::Float(value) => {
            assert_eq!(*value, 3.14);
        }

        value => {
            panic!("Expected Value::Float, got {:?}", value);
        }
    }

    match &row.values()[4] {
        Value::Null => {}

        value => {
            panic!("Expected Value::Null, got {:?}", value);
        }
    }
}

#[test]
fn postgres_ddl() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT,
                surname TEXT
            );
            ",
        ),
        |_connection, _table| {},
    );
}

#[test]
fn postgres_insert() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT,
                surname TEXT
            );
            ",
        ),
        |connection, table| {
            connection
                .exec(&Query::new(&format!(
                    "
                    INSERT INTO {table}
                        (id, name, surname)
                    VALUES
                        (1, 'mario', 'rossi'),
                        (2, 'giovanni', 'andreotti'),
                        (3, 'sandro', 'chiesa');
                    "
                )))
                .unwrap();
        },
    );
}

#[test]
fn postgres_select_multiple_rows() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT,
                surname TEXT
            );
            ",
        ),
        |connection, table| {
            connection
                .exec(&Query::new(&format!(
                    "
                    INSERT INTO {table}
                        (id, name, surname)
                    VALUES
                        (1, 'mario', 'rossi'),
                        (2, 'giovanni', 'andreotti'),
                        (3, 'sandro', 'chiesa');
                    "
                )))
                .unwrap();

            let result = connection
                .exec(&Query::new(&format!("SELECT * FROM {table} ORDER BY id;")))
                .unwrap();

            assert_eq!(result.columns().len(), 3);
            assert_eq!(result.rows().len(), 3);

            match &result.rows()[0].values()[0] {
                Value::Int(value) => {
                    assert_eq!(*value, 1);
                }

                value => {
                    panic!("Expected Value::Int(1), got {:?}", value);
                }
            }

            match &result.rows()[0].values()[1] {
                Value::String(value) => {
                    assert_eq!(value, "mario");
                }

                value => {
                    panic!("Expected Value::String(mario), got {:?}", value);
                }
            }

            match &result.rows()[0].values()[2] {
                Value::String(value) => {
                    assert_eq!(value, "rossi");
                }

                value => {
                    panic!("Expected Value::String(rossi), got {:?}", value);
                }
            }
        },
    );
}

#[test]
fn postgres_update() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT
            );
            ",
        ),
        |connection, table| {
            connection
                .exec(&Query::new(&format!(
                    "
                    INSERT INTO {table}
                        (id, name)
                    VALUES
                        (1, 'mario');
                    "
                )))
                .unwrap();

            connection
                .exec(&Query::new(&format!(
                    "
                    UPDATE {table}
                    SET name = 'luigi'
                    WHERE id = 1;
                    "
                )))
                .unwrap();

            let result = connection
                .exec(&Query::new(&format!(
                    "SELECT name FROM {table} WHERE id = 1;"
                )))
                .unwrap();

            match &result.rows()[0].values()[0] {
                Value::String(value) => {
                    assert_eq!(value, "luigi");
                }

                value => {
                    panic!("Expected Value::String(luigi), got {:?}", value);
                }
            }
        },
    );
}

#[test]
fn postgres_delete() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT
            );
            ",
        ),
        |connection, table| {
            connection
                .exec(&Query::new(&format!(
                    "
                    INSERT INTO {table}
                        (id, name)
                    VALUES
                        (1, 'mario'),
                        (2, 'luigi');
                    "
                )))
                .unwrap();

            connection
                .exec(&Query::new(&format!("DELETE FROM {table} WHERE id = 1;")))
                .unwrap();

            let result = connection
                .exec(&Query::new(&format!("SELECT * FROM {table};")))
                .unwrap();

            assert_eq!(result.rows().len(), 1);

            match &result.rows()[0].values()[0] {
                Value::Int(value) => {
                    assert_eq!(*value, 2);
                }

                value => {
                    panic!("Expected Value::Int(2), got {:?}", value);
                }
            }
        },
    );
}

#[test]
fn postgres_dataframe() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    let result = connection
        .exec(&Query::new(
            "
                SELECT
                    1 AS id,
                    'mario' AS name

                UNION ALL

                SELECT
                    2,
                    'luigi';
                ",
        ))
        .unwrap();

    let _dataframe = result.dataframe().unwrap();
}

#[test]
fn postgres_connection_pool() {
    let config = postgres_config();

    let pool = ConnectionPool::new(config, 2, 5, true).unwrap();

    assert_eq!(pool.total_connections().unwrap(), 2);
    assert_eq!(pool.available_connections().unwrap(), 2);

    {
        let _connection = pool.get().unwrap();

        assert_eq!(pool.available_connections().unwrap(), 1);
    }

    assert_eq!(pool.available_connections().unwrap(), 2);
}

#[test]
fn postgres_connection_pool_wait() {
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

    let config = postgres_config();

    let pool = Arc::new(ConnectionPool::new(config, 1, 1, true).unwrap());

    let connection = pool.get().unwrap();

    assert_eq!(pool.total_connections().unwrap(), 1);
    assert_eq!(pool.available_connections().unwrap(), 0);

    let pool_clone = Arc::clone(&pool);
    let (sender, receiver) = mpsc::channel();

    let handle = thread::spawn(move || {
        let _connection = pool_clone.get_wait().unwrap();
        sender.send(()).unwrap();
    });

    assert!(
        receiver.recv_timeout(Duration::from_millis(100)).is_err(),
        "get_wait() returned before a connection was available"
    );

    drop(connection);

    assert!(
        receiver.recv_timeout(Duration::from_secs(1)).is_ok(),
        "get_wait() did not return after a connection became available"
    );

    handle.join().unwrap();
}

#[test]
fn postgres_transaction_commit() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "transactions",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT
            );
            ",
        ),
        |connection, table| {
            connection.start_transaction().unwrap();

            connection
                .exec(&Query::new(&format!(
                    "INSERT INTO {table} (id, name) VALUES (1, 'mario');"
                )))
                .unwrap();

            connection.commit_transaction().unwrap();

            let result = connection
                .exec(&Query::new(&format!(
                    "SELECT name FROM {table} WHERE id = 1;"
                )))
                .unwrap();

            assert_eq!(result.rows().len(), 1);

            match &result.rows()[0].values()[0] {
                Value::String(value) => {
                    assert_eq!(value, "mario");
                }

                value => {
                    panic!("Expected Value::String(mario), got {:?}", value);
                }
            }
        },
    );
}

#[test]
fn postgres_transaction_rollback() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "transactions",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name TEXT
            );
            ",
        ),
        |connection, table| {
            connection.start_transaction().unwrap();

            connection
                .exec(&Query::new(&format!(
                    "INSERT INTO {table} (id, name) VALUES (1, 'mario');"
                )))
                .unwrap();

            connection.rollback_transaction().unwrap();

            let result = connection
                .exec(&Query::new(&format!("SELECT * FROM {table};")))
                .unwrap();

            assert_eq!(result.rows().len(), 0);
        },
    );
}

#[test]
fn postgres_transaction_state() {
    let config = postgres_config();

    let mut connection = config.connect().unwrap();

    assert!(
        connection.commit_transaction().is_err(),
        "commit should fail outside a transaction"
    );

    assert!(
        connection.rollback_transaction().is_err(),
        "rollback should fail outside a transaction"
    );

    connection.start_transaction().unwrap();

    assert!(
        connection.start_transaction().is_err(),
        "starting a transaction twice should fail"
    );

    connection.rollback_transaction().unwrap();
}

#[test]
fn postgres_pool_rolls_back_open_transaction() {
    let config = postgres_config();

    let pool = ConnectionPool::new(config, 1, 1, true).unwrap();

    let table = unique_table_name("pool_transactions");

    {
        let mut connection = pool.get().unwrap();

        connection
            .exec(&Query::new(&format!(
                "
                CREATE TABLE {table} (
                    id INTEGER PRIMARY KEY
                );
                "
            )))
            .unwrap();
    }

    {
        let mut connection = pool.get().unwrap();

        connection.start_transaction().unwrap();

        connection
            .exec(&Query::new(&format!(
                "INSERT INTO {table} (id) VALUES (1);"
            )))
            .unwrap();

        // Dropping PooledConnection must rollback the transaction.
    }

    {
        let mut connection = pool.get().unwrap();

        let result = connection
            .exec(&Query::new(&format!("SELECT * FROM {table};")))
            .unwrap();

        assert_eq!(result.rows().len(), 0);
    }

    {
        let mut connection = pool.get().unwrap();

        connection
            .exec(&Query::new(&format!("DROP TABLE {table};")))
            .unwrap();
    }
}

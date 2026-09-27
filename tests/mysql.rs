use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use rust_sql::orm::{Schema, select};
use rust_sql::{Connection, ConnectionPool, DatabaseConfig, DatabaseKind, Query, Value};

fn mysql_config() -> DatabaseConfig {
    DatabaseConfig::new(
        DatabaseKind::MySql,
        String::from("localhost"),
        3307,
        String::from("mysql"),
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
fn mysql_connection() {
    let config = mysql_config();

    let connection = config.connect();

    assert!(
        connection.is_ok(),
        "MySQL connection failed: {:?}",
        connection.err()
    );
}

#[test]
fn mysql_simple_select() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    let result = connection.exec(&Query::new("SELECT 1;")).unwrap();

    assert_eq!(result.columns().len(), 1);
    assert_eq!(result.rows().len(), 1);

    match &result.rows()[0].values()[0] {
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
fn mysql_select_values() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    let result = connection
        .exec(&Query::new(
            "
                SELECT
                    1,
                    'hello',
                    TRUE,
                    3.14,
                    NULL;
                ",
        ))
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
            panic!("Expected Value::Int, got {:?}", value);
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
        Value::Int(value) => {
            assert_eq!(*value, 1);
        }

        value => {
            panic!("Expected Value::Int(1), got {:?}", value);
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
fn mysql_ddl() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32),
                surname VARCHAR(32)
            );
            ",
        ),
        |_connection, _table| {},
    );
}

#[test]
fn mysql_insert() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32),
                surname VARCHAR(32)
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
fn mysql_select_multiple_rows() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32),
                surname VARCHAR(32)
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
fn mysql_update() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32)
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
fn mysql_delete() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32)
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
fn mysql_dataframe() {
    let config = mysql_config();

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
fn mysql_connection_pool() {
    let config = mysql_config();

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
fn mysql_connection_pool_wait() {
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

    let config = mysql_config();

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
fn mysql_transaction_commit() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "transactions",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32)
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
fn mysql_transaction_rollback() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "transactions",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32)
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
fn mysql_transaction_state() {
    let config = mysql_config();

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
fn mysql_pool_rolls_back_open_transaction() {
    let config = mysql_config();

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

use rust_sql::{Table, ValueType};

#[derive(Table)]
struct Test {
    #[primary_key]
    id: i64,
    name: String,
    active: bool,
    score: f64,
    data: Vec<u8>,
    test: Option<String>,
}

#[test]
fn derive_table_generates_metadata() {
    assert_eq!(Test::TABLE_NAME, "test");

    let fields = Test::fields();

    assert_eq!(fields.len(), 6);

    assert_eq!(fields[0].name, "id");
    assert!(matches!(fields[0].value_type, ValueType::Int));

    assert_eq!(fields[1].name, "name");
    assert!(matches!(fields[1].value_type, ValueType::String));

    assert_eq!(fields[2].name, "active");
    assert!(matches!(fields[2].value_type, ValueType::Bool));

    assert_eq!(fields[3].name, "score");
    assert!(matches!(fields[3].value_type, ValueType::Float));

    assert_eq!(fields[4].name, "data");
    assert!(matches!(fields[4].value_type, ValueType::Bytes));

    assert_eq!(fields[5].name, "test");
    assert!(matches!(fields[5].value_type, ValueType::String));
    assert!(fields[5].nullable);

    assert!(fields[0].primary_key);

    for (index, field) in fields.iter().enumerate() {
        if index == 0 {
            assert!(field.primary_key);
        } else {
            assert!(!field.primary_key);
        }

        if index == 5 {
            assert!(field.nullable);
        } else {
            assert!(!field.nullable);
        }
    }
}

#[test]
fn derive_table_generates_values() {
    let test = Test {
        id: 42,
        name: String::from("Franco"),
        active: true,
        score: 12.5,
        data: vec![1, 2, 3],
        test: Some(String::from("test")),
    };

    let values = test.values();

    assert_eq!(values.len(), 6);

    assert_eq!(values[0].0, "id");
    assert!(matches!(values[0].1, Value::Int(42)));

    assert_eq!(values[1].0, "name");
    assert!(matches!(
        &values[1].1,
        Value::String(value)
            if value == "Franco"
    ));

    assert_eq!(values[2].0, "active");
    assert!(matches!(values[2].1, Value::Bool(true)));

    assert_eq!(values[3].0, "score");
    assert!(matches!(
        values[3].1,
        Value::Float(value)
            if (value - 12.5).abs() < f64::EPSILON
    ));

    assert_eq!(values[4].0, "data");
    assert!(matches!(
        &values[4].1,
        Value::Bytes(value)
            if value == &vec![1, 2, 3]
    ));

    assert_eq!(values[5].0, "test");
    assert!(matches!(
        &values[5].1,
        Value::String(value)
            if value == "test"
    ));
}

#[test]
fn mysql_prepared_select() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    with_test_table(
        &mut connection,
        "prepared_users",
        String::from(
            "
            CREATE TABLE {table} (
                id INTEGER PRIMARY KEY,
                name VARCHAR(32)
            );
            ",
        ),
        |connection, table| {
            // Insert manuale.
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

            // Un parametro forza exec_prepared().
            let result = connection
                .exec(&Query::with_params(
                    &format!(
                        "
                        SELECT
                            id,
                            name
                        FROM {table}
                        WHERE id = ?;
                        "
                    ),
                    vec![Value::Int(1)],
                ))
                .unwrap();

            assert_eq!(result.columns().len(), 2);
            assert_eq!(result.rows().len(), 1);

            let row = &result.rows()[0];

            assert_eq!(row.size(), 2);

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
fn mysql_orm_select() {
    let config = mysql_config();

    let mut connection = config.connect().unwrap();

    connection
        .exec(&Query::new(&format!(
            "DROP TABLE IF EXISTS {};",
            Test::TABLE_NAME
        )))
        .unwrap();

    Schema::new(&config)
        .unwrap()
        .table::<Test>()
        .create_all()
        .unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| {
        connection
            .exec(&Query::new(&format!(
                "
                INSERT INTO {} (
                    id,
                    name,
                    active,
                    score,
                    data,
                    test
                )
                VALUES
                    (1, 'mario', TRUE, 10.0, X'010203', NULL),
                    (2, 'luigi', TRUE, 20.0, X'040506', 'a'),
                    (3, 'peach', FALSE, 30.0, X'070809', 'b'),
                    (4, 'toad', TRUE, 5.0, X'0A0B0C', NULL);
                ",
                Test::TABLE_NAME
            )))
            .unwrap();

        let query = select::<Test>()
            .where_clause(Test::id.gt(1))
            .where_clause(
                Test::score
                    .gte(20)
                    .or(Test::name.eq("mario")),
            )
            .order_by(Test::id.desc())
            .limit(2);

        let result = connection.exec(&query).unwrap();

        assert_eq!(result.len(), 2);

        let first = &result[0];
        let second = &result[1];

        assert_eq!(first.id, 3);
        assert_eq!(first.name, "peach");
        assert!(!first.active);
        assert_eq!(first.score, 30.0);
        assert_eq!(first.data, vec![0x07, 0x08, 0x09]);
        assert_eq!(first.test.as_deref(), Some("b"));

        assert_eq!(second.id, 2);
        assert_eq!(second.name, "luigi");
        assert!(second.active);
        assert_eq!(second.score, 20.0);
        assert_eq!(second.data, vec![0x04, 0x05, 0x06]);
        assert_eq!(second.test.as_deref(), Some("a"));
    }));

    connection
        .exec(&Query::new(&format!(
            "DROP TABLE IF EXISTS {};",
            Test::TABLE_NAME
        )))
        .unwrap();

    if let Err(payload) = result {
        resume_unwind(payload);
    }
}
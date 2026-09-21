# rust-sql

A lightweight SQL client library written in Rust, with support for PostgreSQL and MySQL.

The project implements database communication directly through the native wire protocols, without relying on an external database client library.

## Features

* PostgreSQL support
* MySQL support
* Native TCP communication
* PostgreSQL authentication and SCRAM-SHA-256
* MySQL authentication, including `caching_sha2_password`
* SQL query execution
* Typed query results
* Common result representation for PostgreSQL and MySQL
* `Dataframe` conversion for tabular results
* Connection pooling
* Session-based query execution
* No runtime dependency on the PostgreSQL or MySQL client libraries

## Supported databases

| Database   | Status    |
| ---------- | --------- |
| PostgreSQL | Supported |
| MySQL      | Supported |

The library currently focuses on the classic/native wire protocols and provides a common API above the database-specific implementations.

## Example

```rust
use rust_sql::{DatabaseConfig, DatabaseKind, Query};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DatabaseConfig::new(
        DatabaseKind::Postgres,
        "127.0.0.1".to_string(),
        5432,
        "postgres".to_string(),
        "password".to_string(),
        "example".to_string(),
    );

    let mut connection = config.connect()?;

    let result = connection.exec(&Query::new(
        "SELECT id, name FROM users;",
    ))?;

    for row in result.rows() {
        println!("{row}");
    }

    Ok(())
}
```

## Results

Query results expose column metadata and rows through a database-independent representation.

```rust
let result = connection.exec(&Query::new(
    "SELECT id, name FROM users;",
))?;

for column in result.columns() {
    println!("{}", column.name);
}

for row in result.rows() {
    println!("{row}");
}
```

Values are represented by the `Value` enum and include:

* `Null`
* `Int`
* `UInt`
* `Float`
* `String`
* `Bytes`
* `Bool`

Results can also be converted into a `Dataframe`:

```rust
let dataframe = result.dataframe()?;
println!("{dataframe}");
```

## Connection pooling

The library provides a connection pool and session abstraction for applications that need to reuse database connections.

```rust
use rust_sql::{DatabaseConfig, DatabaseKind, Query, SessionMaker};

let config = DatabaseConfig::new(
    DatabaseKind::Postgres,
    "127.0.0.1".to_string(),
    5432,
    "postgres".to_string(),
    "password".to_string(),
    "example".to_string(),
);

let sessions = SessionMaker::new(config)?;

let session = sessions.session();

let result = session.exec(&Query::new(
    "SELECT id, name FROM users;",
))?;
```

Connections are returned to the pool automatically when a pooled connection is dropped.

## Architecture

The library is organized around a common database abstraction with database-specific protocol implementations.

```text
Connection
    │
    ├── PostgreSQL backend
    │       └── PostgreSQL wire protocol
    │
    └── MySQL backend
            └── MySQL wire protocol
```

The main components are:

```text
src/
├── backend/
│   ├── mod.rs
│   ├── postgres.rs
│   └── mysql.rs
├── mysql_protocol/
│   ├── mod.rs
│   ├── message.rs
│   ├── result.rs
│   └── authentication.rs
├── postgres_protocol/
│   ├── mod.rs
│   ├── message.rs
│   ├── result.rs
│   ├── authentication.rs
│   └── scram.rs
├── config.rs
├── connection.rs
├── query.rs
├── table.rs
└── lib.rs
```

`Connection` provides the public connection API, while the backend implementations handle database-specific behavior. Protocol modules are responsible for encoding and decoding wire-protocol messages.

## Current status

The project is currently in early development.

The `0.1.0` release provides the initial PostgreSQL and MySQL client implementation, including authentication, query execution, result parsing, connection pooling, and integration tests.

The API should still be considered subject to change before the project reaches a more mature release.

## Development

Run the test suite with:

```bash
cargo test
```

Run Clippy with warnings treated as errors:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Check formatting:

```bash
cargo fmt -- --check
```

Build the API documentation:

```bash
cargo doc --no-deps
```

Integration tests require running PostgreSQL and MySQL instances with the credentials expected by the test configuration.

## License

This project is licensed under the MIT License.

See the [LICENSE](LICENSE) file for the complete license text.

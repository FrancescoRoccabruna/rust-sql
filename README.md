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
* Prepared queries with parameters
* Typed query results
* Common result representation for PostgreSQL and MySQL
* `Dataframe` conversion for tabular results
* Connection pooling
* Session-based query execution
* Typed ORM queries
* `Table` derive for mapping Rust structs to database tables
* Schema creation from Rust table definitions
* Foreign key constraints
* ORM relationships with forward and back-reference navigation
* Session identity map
* Unit of Work with tracked entity states
* Automatic INSERT, UPDATE, and DELETE handling
* Dirty tracking for persistent entities
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

let mut session = sessions.session();

let result = session.exec(&Query::new(
    "SELECT id, name FROM users;",
))?;
```

Connections are returned to the pool automatically when a pooled connection is dropped.

## ORM

Rust structs can be mapped to database tables using the `Table` derive, including primary keys, foreign keys, and ORM relationships.

```rust
use rust_sql::Table;

#[derive(Table)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
    active: bool,
}
```

Tables can be created from their Rust definitions:

```rust
use rust_sql::orm::Schema;

Schema::new(&config)?
    .table::<User>()
    .create_all()?;
```

Foreign keys can be declared directly on table fields, with optional ORM relationship metadata:

```rust
#[derive(Table)]
struct User {
    #[primary_key]
    id: i64,
    name: String,
}

#[derive(Table)]
struct Post {
    #[primary_key]
    id: i64,

    #[foreign_key(
        User::id,
        relationship = user,
        backref = posts
    )]
    user_id: i64,

    title: String,
}
```

The foreign key is used both for schema generation and, when relationship metadata is provided, for ORM navigation.

Relationships are loaded through a session:

```rust
use rust_sql::orm::select;

let post = session
    .exec(
        &select::<Post>()
            .where_clause(Post::id.eq(1))
    )?
    .first()
    .unwrap();

let user = post
    .user(&mut session)?
    .unwrap();

let posts = user.posts(&mut session)?;
```

Typed queries can be executed directly through a connection:

```rust
use rust_sql::orm::select;

let users = connection
    .exec(
        &select::<User>()
            .where_clause(User::active.eq(true))
            .order_by(User::id.asc())
            .limit(10),
    )?
    .all();
```

Sessions additionally track ORM entities through an identity map and Unit of Work.

```rust
let sessions = SessionMaker::new(config)?;
let mut session = sessions.session();

let user = session.add(User {
    id: 1,
    name: String::from("Mario"),
    active: true,
})?;

session.commit()?;

user.write().name = String::from("Luigi");

session.commit()?;
```

Tracked entities can also be deleted through the session:

```rust
session.delete(user)?;
session.commit()?;
```

A session tracks entity state and automatically determines whether an INSERT, UPDATE, or DELETE is required when `commit()` is called.

Primary keys are immutable after an entity becomes tracked by a session.

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

Session
    │
    ├── Connection pool
    ├── Identity map
    ├── Relationships
    └── Unit of Work
            ├── INSERT
            ├── UPDATE
            └── DELETE
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
├── orm/
├── config.rs
├── connection.rs
├── query.rs
├── table.rs
└── lib.rs
```

`Connection` provides direct database access and raw or typed query execution.

`Session` builds on top of the connection pool and provides stateful ORM behavior, including identity tracking, relationship loading, and Unit of Work management.

Backend implementations handle database-specific behavior, while protocol modules are responsible for encoding and decoding wire-protocol messages.

## Current status

The project is currently in early development.

The `0.1.0` release introduced the initial PostgreSQL and MySQL client implementation, including authentication, query execution, result parsing, and the core database abstraction.

The `0.2.0` release expands the library with connection pooling and the first ORM layer, including typed table mappings, schema creation, typed queries, sessions, identity tracking, Unit of Work behavior, dirty tracking, and automatic INSERT, UPDATE, and DELETE operations.

The `0.3.0` release adds foreign key support and ORM relationships. Foreign keys can be declared directly through `Table` metadata and are included in generated schemas for both PostgreSQL and MySQL. Relationships can be defined on top of foreign keys with forward navigation, back-references, nullable relationships, and integration with the session identity map.

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
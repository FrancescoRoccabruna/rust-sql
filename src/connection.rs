use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::TcpStream,
    ops::{Deref, DerefMut},
    sync::{Arc, Condvar, Mutex},
};

use crate::{
    DatabaseConfig,
    backend::{Backend, mysql::MysqlBackend, postgres::PostgresBackend},
    config::DatabaseKind,
    query::{Query, QueryResult},
};

#[expect(dead_code)]
enum ConnectionState {
    Ready,
    Busy,
    InTransaction,
    Error,
}

/// A connection to a database server.
pub struct Connection {
    host: String,
    port: u16,
    kind: DatabaseKind,
    stream: Option<TcpStream>,
    state: ConnectionState,
}

impl Connection {
    /// Creates a new database connection configuration.
    pub fn new(host: String, port: u16, kind: DatabaseKind) -> Self {
        Self {
            host,
            port,
            kind,
            stream: None,
            state: ConnectionState::Ready,
        }
    }

    fn backend(&mut self) -> Box<dyn Backend + '_> {
        match &self.kind {
            DatabaseKind::Postgres => Box::new(PostgresBackend::new(self)),

            DatabaseKind::MySql => Box::new(MysqlBackend::new(self)),
        }
    }

    fn connect_tcp(&mut self) -> Result<(), DbError> {
        let address = format!("{}:{}", self.host, self.port);

        let stream =
            TcpStream::connect(address).map_err(|error| DbError::new(error.to_string()))?;

        self.stream = Some(stream);

        Ok(())
    }

    /// Starts a new database transaction.
    pub fn start_transaction(&mut self) -> Result<(), DbError> {
        match &self.state {
            ConnectionState::Busy => return Err(DbError::new(String::from("Connection is busy"))),
            ConnectionState::Error => return Err(DbError::new(String::from("Connection error"))),
            ConnectionState::InTransaction => {
                return Err(DbError::new(String::from(
                    "Connection is already in a transaction",
                )));
            }

            _ => {}
        }

        {
            let mut backend = self.backend();
            backend.start_transaction()?;
        }

        self.state = ConnectionState::InTransaction;

        Ok(())
    }

    /// Commits the current database transaction.
    pub fn commit_transaction(&mut self) -> Result<(), DbError> {
        match &self.state {
            ConnectionState::Busy => return Err(DbError::new(String::from("Connection is busy"))),
            ConnectionState::Error => return Err(DbError::new(String::from("Connection error"))),
            ConnectionState::Ready => {
                return Err(DbError::new(String::from(
                    "Connection is not in a transaction",
                )));
            }

            _ => {}
        }

        {
            let mut backend = self.backend();
            backend.commit_transaction()?;
        }

        self.state = ConnectionState::Ready;

        Ok(())
    }

    /// Rolls back the current database transaction.
    pub fn rollback_transaction(&mut self) -> Result<(), DbError> {
        match &self.state {
            ConnectionState::Busy => return Err(DbError::new(String::from("Connection is busy"))),
            ConnectionState::Error => return Err(DbError::new(String::from("Connection error"))),
            ConnectionState::Ready => {
                return Err(DbError::new(String::from(
                    "Connection is not in a transaction",
                )));
            }

            _ => {}
        }

        {
            let mut backend = self.backend();
            backend.rollback_transaction()?;
        }

        self.state = ConnectionState::Ready;

        Ok(())
    }

    /// Opens the connection and authenticates with the database server.
    pub fn open(&mut self, username: &str, password: &str, db_name: &str) -> Result<(), DbError> {
        self.connect_tcp()?;

        let mut backend = self.backend();
        backend.open(username, password, db_name)
    }

    /// Returns `true` if the underlying TCP connection is open.
    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    pub(crate) fn write(&mut self, data: &[u8]) -> Result<(), DbError> {
        match &mut self.stream {
            Some(stream) => {
                stream
                    .write_all(data)
                    .map_err(|e| DbError::new(e.to_string()))?;

                Ok(())
            }
            None => Err(DbError::new(String::from("Connection is not open"))),
        }
    }

    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> Result<(), DbError> {
        let stream = match &mut self.stream {
            Some(stream) => stream,
            None => {
                return Err(DbError::new(String::from("Connection is not open")));
            }
        };

        stream
            .read_exact(buffer)
            .map_err(|e| DbError::new(e.to_string()))?;

        Ok(())
    }

    /// Executes a SQL query.
    pub fn exec(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        match &self.state {
            ConnectionState::Busy => return Err(DbError::new(String::from("Connection is busy"))),
            ConnectionState::Error => return Err(DbError::new(String::from("Connection error"))),

            _ => {}
        }

        let in_transaction = matches!(self.state, ConnectionState::InTransaction);

        self.state = ConnectionState::Busy;

        let result = {
            let mut backend = self.backend();
            backend.exec(query)
        };

        self.state = if in_transaction {
            ConnectionState::InTransaction
        } else {
            ConnectionState::Ready
        };

        result
    }
}

/// Error returned by database operations.
#[derive(Debug)]
pub struct DbError {
    pub message: String,
}

impl DbError {
    /// Creates a new database error.
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DbError {}

struct PoolInner {
    connections: VecDeque<Connection>,
    total_connections: usize,
}

/// A pool of reusable database connections.
#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<(Mutex<PoolInner>, Condvar)>,
    config: Arc<DatabaseConfig>,
    expand: bool,
    max_connections: usize,
}

impl ConnectionPool {
    /// Creates a new connection pool.
    pub fn new(
        config: DatabaseConfig,
        default_connections: usize,
        max_connections: usize,
        expand: bool,
    ) -> Result<Self, DbError> {
        let mut connections = VecDeque::with_capacity(max_connections);

        if max_connections != 0 && default_connections > max_connections {
            return Err(DbError::new(String::from(
                "default connections exceeded max connections",
            )));
        }

        for _ in 0..default_connections {
            let connection = config.connect()?;

            connections.push_back(connection);
        }

        Ok(Self {
            inner: Arc::new((
                Mutex::new(PoolInner {
                    connections,
                    total_connections: default_connections,
                }),
                Condvar::new(),
            )),
            config: Arc::new(config),
            expand,
            max_connections,
        })
    }

    /// Acquires a connection from the pool.
    pub fn get(&self) -> Result<PooledConnection, DbError> {
        let mut inner = self
            .inner
            .0
            .lock()
            .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

        if self.expand
            && inner.connections.is_empty()
            && (inner.total_connections < self.max_connections || self.max_connections == 0)
        {
            drop(inner);

            let connection = self.config.connect()?;

            inner = self
                .inner
                .0
                .lock()
                .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

            if inner.total_connections < self.max_connections || self.max_connections == 0 {
                inner.connections.push_back(connection);
                inner.total_connections += 1;
            }
        }

        let conn = inner
            .connections
            .pop_front()
            .ok_or_else(|| DbError::new(String::from("No available connection")))?;

        Ok(PooledConnection {
            conn: Some(conn),
            inner: Arc::clone(&self.inner),
        })
    }

    /// Acquires a connection from the pool, waiting if none are available.
    pub fn get_wait(&self) -> Result<PooledConnection, DbError> {
        let mut inner = self
            .inner
            .0
            .lock()
            .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

        if self.expand
            && inner.connections.is_empty()
            && (inner.total_connections < self.max_connections || self.max_connections == 0)
        {
            drop(inner);

            let connection = self.config.connect()?;

            inner = self
                .inner
                .0
                .lock()
                .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

            if inner.total_connections < self.max_connections || self.max_connections == 0 {
                inner.connections.push_back(connection);
                inner.total_connections += 1;
            }
        }

        loop {
            if let Some(conn) = inner.connections.pop_front() {
                return Ok(PooledConnection {
                    conn: Some(conn),
                    inner: Arc::clone(&self.inner),
                });
            }

            inner = self
                .inner
                .1
                .wait(inner)
                .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;
        }
    }

    /// Returns the number of currently available connections.
    pub fn available_connections(&self) -> Result<usize, DbError> {
        let inner = self
            .inner
            .0
            .lock()
            .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

        Ok(inner.connections.len())
    }

    /// Returns the total number of connections managed by the pool.
    pub fn total_connections(&self) -> Result<usize, DbError> {
        let inner = self
            .inner
            .0
            .lock()
            .map_err(|_| DbError::new(String::from("Connection pool lock error")))?;

        Ok(inner.total_connections)
    }
}

/// A database connection borrowed from a [`ConnectionPool`].
pub struct PooledConnection {
    conn: Option<Connection>,
    inner: Arc<(Mutex<PoolInner>, Condvar)>,
}

impl Deref for PooledConnection {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().unwrap()
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        let Some(mut conn) = self.conn.take() else {
            return;
        };

        if matches!(conn.state, ConnectionState::InTransaction)
            && conn.rollback_transaction().is_err()
        {
            return;
        }

        if let Ok(mut inner) = self.inner.0.lock() {
            inner.connections.push_back(conn);
            self.inner.1.notify_one();
        }
    }
}

/// A session that executes queries using connections from a pool.
pub struct Session {
    pool: Arc<ConnectionPool>,
    statements: Vec<Query>,
}

impl Session {
    fn new(pool: Arc<ConnectionPool>) -> Self {
        Self {
            pool,
            statements: Vec::new(),
        }
    }

    /// Executes a SQL query.
    pub fn exec(&self, query: &Query) -> Result<QueryResult, DbError> {
        let mut connection = self.pool.get_wait()?;

        connection.exec(query)
    }

    /// Adds a SQL query to the current unit of work.
    pub fn add(&mut self, query: Query) {
        self.statements.push(query);
    }

    /// Commits all pending queries in a single database transaction.
    pub fn commit(&mut self) -> Result<(), DbError> {
        if !self.statements.is_empty() {
            let mut connection = self.pool.get_wait()?;

            connection.start_transaction()?;

            for query in &self.statements {
                connection.exec(query)?;
            }

            connection.commit_transaction()?;
        }

        self.statements.clear();

        Ok(())
    }
}

/// Creates sessions backed by a connection pool.
pub struct SessionMaker {
    pool: Arc<ConnectionPool>,
}

impl SessionMaker {
    /// Creates a new session maker backed by a connection pool.
    pub fn new(config: DatabaseConfig) -> Result<Self, DbError> {
        let pool = ConnectionPool::new(config, 2, 5, true)?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Creates a new database session.
    pub fn session(&self) -> Session {
        Session::new(self.pool.clone())
    }
}

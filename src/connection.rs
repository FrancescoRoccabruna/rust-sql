use std::{
    any::{Any, TypeId}, cell::RefCell, collections::{HashMap, VecDeque}, hash::Hash, io::{Read, Write}, net::TcpStream, ops::{Deref, DerefMut}, rc::Rc, sync::{Arc, Condvar, Mutex},
};

use crate::{
    DatabaseConfig, Value, backend::{Backend, mysql::MysqlBackend, postgres::PostgresBackend}, config::DatabaseKind, orm::{InsertQuery, Table, TableDefinition, query::{self, Entity, InsertQueryRequest, SelectQueryRequest, SessionExecutableQuery, UpdateQuery, UpdateQueryRequest}}, query::{ExecutableQuery, Query, QueryResult},
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
    pub fn exec<Q>(&mut self, query: &Q) -> Result<Q::Output, DbError>
    where
        Q: ExecutableQuery,
    {
        match &self.state {
            ConnectionState::Busy => return Err(DbError::new(String::from("Connection is busy"))),
            ConnectionState::Error => return Err(DbError::new(String::from("Connection error"))),

            _ => {}
        }

        let in_transaction = matches!(self.state, ConnectionState::InTransaction);

        self.state = ConnectionState::Busy;

        let result = { query.execute(self) };

        self.state = if in_transaction {
            ConnectionState::InTransaction
        } else {
            ConnectionState::Ready
        };

        result
    }

    pub(crate) fn execute_raw(&mut self, query: &Query) -> Result<QueryResult, DbError> {
        let mut backend = self.backend();
        backend.exec(query)
    }

    pub(crate) fn execute_orm(
        &mut self,
        query: &dyn SelectQueryRequest,
    ) -> Result<QueryResult, DbError> {
        let mut backend = self.backend();
        backend.exec_orm(query)
    }

    pub(crate) fn create_table(&mut self, table: &TableDefinition) -> Result<(), DbError> {
        let mut backend = self.backend();

        backend.create_table(table)
    }

    pub(crate) fn execute_insert(
        &mut self,
        query: &dyn InsertQueryRequest,
    ) -> Result<(), DbError> {
        let mut backend = self.backend();
        backend.exec_insert(query)
    }

    pub(crate) fn execute_update(
        &mut self,
        query: &dyn UpdateQueryRequest,
    ) -> Result<(), DbError> {
        let mut backend = self.backend();
        backend.exec_update(query)
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
    identity_map: HashMap<IdentityKey, Box<dyn EntityEntry>>,
}

impl Session {
    fn new(pool: Arc<ConnectionPool>) -> Self {
        Self {
            pool,
            identity_map: HashMap::new(),
        }
    }

    /// Executes a SQL query.
    pub fn exec<Q>(&mut self, query: &Q) -> Result<Q::Output, DbError>
    where
        Q: SessionExecutableQuery,
    {
        query.execute_in_session(self)
    }

    /// Adds a SQL query to the current unit of work.
    pub fn add<T>(
        &mut self,
        record: T,
    ) -> Result<Entity<T>, DbError>
    where
        T: Table + 'static,
    {
        self.track_pending(record)
    }

    /// Commits all pending queries in a single database transaction.
    pub fn commit(&mut self) -> Result<(), DbError> {
        if self.identity_map.is_empty() {
            return Ok(());
        }

        let mut connection = self.pool.get_wait()?;
        connection.start_transaction()?;

        for entry in self.identity_map.values_mut() {
            if let Err(error) = entry.flush(&mut connection) {
                let _ = connection.rollback_transaction();
                return Err(error);
            }
        }

        if let Err(error) = connection.commit_transaction() {
            let _ = connection.rollback_transaction();
            return Err(error);
        }

        for entry in self.identity_map.values_mut() {
            entry.commit_flush();
        }

        Ok(())
    }

    pub(crate) fn execute_select(
        &self,
        query: &dyn SelectQueryRequest,
    ) -> Result<QueryResult, DbError> {
        let mut connection = self.pool.get_wait()?;
        connection.execute_orm(query)
    }

    pub(crate) fn execute_query<Q>(
        &self,
        query: &Q,
    ) -> Result<Q::Output, DbError>
    where
        Q: ExecutableQuery,
    {
        let mut connection = self.pool.get_wait()?;
        connection.exec(query)
    }
}

impl Session {
    fn get_tracked<T>(
        &self,
        key: &IdentityKey,
    ) -> Option<Rc<RefCell<T>>>
    where
        T: Table + 'static,
    {
        let entry = self.identity_map.get(key)?;

        let tracked = entry
            .as_any()
            .downcast_ref::<Tracked<T>>()?;

        Some(Rc::clone(&tracked.record))
    }


    pub(crate) fn track_persistent<T>(
        &mut self,
        record: T,
    ) -> Result<Rc<RefCell<T>>, DbError>
    where
        T: Table + 'static,
    {
        let key = IdentityKey::from_record(&record)?;

        if let Some(existing) = self.get_tracked::<T>(&key) {
            return Ok(existing);
        }

        let original_values = record.values();
        let record = Rc::new(RefCell::new(record));

        self.identity_map.insert(
            key,
            Box::new(Tracked {
                record: Rc::clone(&record),
                state: EntityState::Persistent,
                original_values,
            }),
        );

        Ok(record)
    }

    pub(crate) fn track_pending<T>(
        &mut self,
        record: T,
    ) -> Result<Entity<T>, DbError>
    where
        T: Table + 'static,
    {
        let key = IdentityKey::from_record(&record)?;

        if self.identity_map.contains_key(&key) {
            return Err(DbError::new(String::from(
                "Entity is already tracked by this session",
            )));
        }

        let record = Rc::new(RefCell::new(record));

        self.identity_map.insert(
            key,
            Box::new(Tracked {
                record: Rc::clone(&record),
                state: EntityState::Pending,
                original_values: Vec::new(),
            }),
        );

        Ok(Entity::from_inner(record))
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

#[derive(Debug, Hash, PartialEq, Eq)]
struct IdentityKey {
    type_id: TypeId,
    primary_key: IdentityValue,
}

impl IdentityKey {
    fn from_record<T>(record: &T) -> Result<Self, DbError>
    where
        T: Table + 'static,
    {
        let primary_key = T::fields()
            .into_iter()
            .find(|field| field.primary_key)
            .ok_or_else(|| {
                DbError::new(String::from("Primary key not found"))
            })?;

        let value = record
            .values()
            .into_iter()
            .find(|(name, _)| *name == primary_key.name)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                DbError::new(String::from(
                    "Primary key value not found",
                ))
            })?;

        Ok(Self {
            type_id: TypeId::of::<T>(),
            primary_key: IdentityValue::try_from(&value)?,
        })
    }
}



#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum IdentityValue {
    Int(i64),
    UInt(u64),
    String(String),
    Bytes(Vec<u8>),
}

impl TryFrom<&Value> for IdentityValue {
    type Error = DbError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Int(value) => Ok(Self::Int(*value)),
            Value::UInt(value) => Ok(Self::UInt(*value)),
            Value::String(value) => Ok(Self::String(value.clone())),
            Value::Bytes(value) => Ok(Self::Bytes(value.clone())),

            Value::Null => Err(DbError::new(
                String::from("Primary key cannot be null")
            )),

            value => Err(DbError::new(format!(
                "Unsupported primary key value: {:?}",
                value,
            ))),
        }
    }
}

#[derive(Clone, Copy)]
enum EntityState {
    Pending,
    Persistent,
    Deleted,
}

trait EntityEntry {
    fn state(&self) -> EntityState;
    fn flush(&mut self, connection: &mut Connection) -> Result<(), DbError>;

    fn commit_flush(&mut self);
    fn as_any(&self) -> &dyn Any;
}

struct Tracked<T: Table> {
    record: Rc<RefCell<T>>,
    state: EntityState,
    original_values: Vec<(&'static str, Value)>,
}

fn primary_key_value<'a, T: Table>(
    values: &'a [(&'static str, Value)],
) -> Option<&'a Value> {
    let pk = T::fields()
        .into_iter()
        .find(|field| field.primary_key)?;

    values
        .iter()
        .find(|(name, _)| *name == pk.name)
        .map(|(_, value)| value)
}

impl<T> EntityEntry for Tracked<T>
where
    T: Table + 'static,
{
    fn state(&self) -> EntityState {
        self.state
    }

    fn flush(
        &mut self,
        connection: &mut Connection,
    ) -> Result<(), DbError> {
        match self.state {
            EntityState::Pending => {
                let record = self.record.borrow();

                let query = InsertQuery::new(&*record);

                connection.execute_insert(&query)?;

                Ok(())
            }

            EntityState::Persistent => {
                let current_values = self.record.borrow().values();

                if current_values == self.original_values {
                    return Ok(());
                }

                let original_pk = primary_key_value::<T>(&self.original_values)
                    .ok_or_else(|| DbError::new(
                        String::from("Primary key not found in original values")
                    ))?;

                let current_pk = primary_key_value::<T>(&current_values)
                    .ok_or_else(|| DbError::new(
                        String::from("Primary key not found in current values")
                    ))?;

                if original_pk != current_pk {
                    return Err(DbError::new(String::from(
                        "Primary key of a persistent entity cannot be changed",
                    )));
                }

                {
                    let record = self.record.borrow();
                    let query = UpdateQuery::new(&*record);

                    connection.execute_update(&query)?;
                }

                Ok(())
            },

            EntityState::Deleted => {
                todo!()
            }
        }
    }

    fn commit_flush(&mut self) {
        match self.state {
            EntityState::Pending => {
                self.original_values = self.record.borrow().values();
                self.state = EntityState::Persistent;
            }

            EntityState::Persistent => {
                self.original_values = self.record.borrow().values();
            }

            EntityState::Deleted => {}
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}




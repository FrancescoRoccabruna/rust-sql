use crate::{
    Connection, DbError, Session, Value, orm::{Table, column::ColumnRef}, query::ExecutableQuery,
};
use std::{any::TypeId, cell::{Ref, RefCell, RefMut}, marker::PhantomData, rc::Rc};

/// Represents a select query for an ORM table.
pub struct SelectQuery<T: Table> {
    _marker: PhantomData<T>,
    where_clause: Vec<Condition>,
    order_by: Vec<OrderBy>,
    limit: Option<usize>,
}

/// Creates a new ORM `SELECT` query for the specified table.
pub fn select<T: Table>() -> SelectQuery<T> {
    SelectQuery {
        _marker: PhantomData,
        where_clause: Vec::new(),
        order_by: Vec::new(),
        limit: None,
    }
}

impl<T: Table> SelectQuery<T> {
    /// Adds a condition to the query.
    pub fn where_clause(mut self, condition: Condition) -> Self {
        self.where_clause.push(condition);
        self
    }

    /// Adds an ordering expression to the query.
    pub fn order_by(mut self, order_by: OrderBy) -> Self {
        self.order_by.push(order_by);
        self
    }

    /// Limits the maximum number of rows returned by the query.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

pub(crate) trait SelectQueryRequest {
    fn table_name(&self) -> &'static str;
    fn fields(&self) -> Vec<ColumnRef>;
    fn where_clause(&self) -> &[Condition];
    fn order_by(&self) -> &[OrderBy];
    fn limit(&self) -> Option<usize>;
}

impl<T: Table> SelectQueryRequest for SelectQuery<T> {
    fn table_name(&self) -> &'static str {
        T::table_name()
    }

    fn fields(&self) -> Vec<ColumnRef> {
        T::fields()
    }

    fn where_clause(&self) -> &[Condition] {
        &self.where_clause
    }

    fn order_by(&self) -> &[OrderBy] {
        &self.order_by
    }

    fn limit(&self) -> Option<usize> {
        self.limit
    }
}

pub(crate) trait InsertQueryRequest {
    fn table_name(&self) -> &'static str;
    fn values(&self) -> Vec<(&'static str, Value)>;
}

pub(crate) trait UpdateQueryRequest {
    fn table_name(&self) -> &'static str;
    fn primary_key(&self) -> (&'static str, Value);
    fn values(&self) -> Vec<(&'static str, Value)>;
}

pub(crate) trait DeleteQueryRequest {
    fn table_name(&self) -> &'static str;
    fn primary_key(&self) -> (&'static str, Value);
}

pub struct SelectResult<T: Table> {
    result: Vec<T>,
}

pub struct TrackedSelectResult<T: Table> {
    result: Vec<Entity<T>>,
}

impl<T: Table> SelectResult<T> {
    pub fn all(self) -> Vec<T> {
        self.result
    }

    pub fn first(self) -> Option<T> {
        self.result.into_iter().next()
    }

    pub fn len(&self) -> usize {
        self.result.len()
    }

    pub fn is_empty(&self) -> bool {
        self.result.is_empty()
    }
}

impl<T: Table> TrackedSelectResult<T> {
    pub fn all(self) -> Vec<Entity<T>> {
        self.result
    }

    pub fn first(self) -> Option<Entity<T>> {
        self.result.into_iter().next()
    }

    pub fn len(&self) -> usize {
        self.result.len()
    }

    pub fn is_empty(&self) -> bool {
        self.result.is_empty()
    }
}

impl<T> ExecutableQuery for SelectQuery<T>
where
    T: Table,
{
    type Output = SelectResult<T>;

    fn execute(
        &self,
        connection: &mut Connection,
    ) -> Result<Self::Output, DbError> {
        let result = connection.execute_orm(self)?;

        let result = result
            .rows()
            .iter()
            .map(T::from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SelectResult { result })
    }
}

/// Represents a condition applied to an ORM query.
pub enum Condition {
    /// Column equals a value.
    Eq(ColumnRef, Value),

    /// Column does not equal a value.
    NotEq(ColumnRef, Value),

    /// Column is greater than a value.
    Gt(ColumnRef, Value),

    /// Column is greater than or equal to a value.
    Gte(ColumnRef, Value),

    /// Column is less than a value.
    Lt(ColumnRef, Value),

    /// Column is less than or equal to a value.
    Lte(ColumnRef, Value),

    /// Column is NULL.
    IsNull(ColumnRef),

    /// Column is not NULL.
    IsNotNull(ColumnRef),

    /// Combines two conditions with a logical AND.
    And(Box<Condition>, Box<Condition>),

    /// Combines two conditions with a logical OR.
    Or(Box<Condition>, Box<Condition>),
}

impl Condition {
    /// Combines two conditions with a logical AND.
    pub fn and(self, other: Condition) -> Condition {
        Condition::And(Box::new(self), Box::new(other))
    }

    /// Combines two conditions with a logical OR.
    pub fn or(self, other: Condition) -> Condition {
        Condition::Or(Box::new(self), Box::new(other))
    }
}

/// Represents an ordering expression for an ORM query.
pub enum OrderBy {
    /// Sorts the column in ascending order.
    Asc(ColumnRef),

    /// Sorts the column in descending order.
    Desc(ColumnRef),
}

/// Represents an insert query for an ORM table.
pub(crate) struct InsertQuery<'a, T: Table> {
    record: &'a T,
}

impl<'a, T: Table> InsertQuery<'a, T> {
    pub(crate) fn new(record: &'a T) -> Self {
        Self { record }
    }
}

impl<T: Table> InsertQueryRequest for InsertQuery<'_, T> {
    fn table_name(&self) -> &'static str {
        T::table_name()
    }

    fn values(&self) -> Vec<(&'static str, Value)> {
        self.record.values()
    }
}

/// Represents an update query for an ORM table.
pub(crate) struct UpdateQuery<'a, T: Table> {
    record: &'a T,
}

impl<'a, T: Table> UpdateQuery<'a, T> {
    pub(crate) fn new(record: &'a T) -> Self {
        Self { record }
    }
}

/// Represents a delete query for an ORM table.
pub(crate) struct DeleteQuery<'a, T: Table> {
    record: &'a T,
}

impl<'a, T: Table> DeleteQuery<'a, T> {
    pub(crate) fn new(record: &'a T) -> Self {
        Self { record }
    }
}

impl<T: Table> UpdateQueryRequest for UpdateQuery<'_, T> {
    fn table_name(&self) -> &'static str {
        T::table_name()
    }

    fn values(&self) -> Vec<(&'static str, Value)> {
        self.record.values()
    }

    fn primary_key(&self) -> (&'static str, Value) {
        let fields = T::fields();
        let values = self.record.values();

        let pk = fields
            .iter()
            .find(|field| field.primary_key)
            .expect("primary key required");

        let (_, value) = values
            .into_iter()
            .find(|(name, _)| *name == pk.name)
            .expect("primary key value required");

        (pk.name, value)
    }
}

pub trait SessionExecutableQuery {
    type Output;

    fn execute_in_session(
        &self,
        session: &mut Session,
    ) -> Result<Self::Output, DbError>;
}

impl<T> SessionExecutableQuery for SelectQuery<T>
where
    T: Table + 'static,
{
    type Output = TrackedSelectResult<T>;

    fn execute_in_session(
        &self,
        session: &mut Session,
    ) -> Result<Self::Output, DbError> {
        let result = session.execute_select(self)?;

        let mut records = Vec::new();

        for row in result.rows() {
            let record = T::from_row(row)?;
            let tracked = session.track_persistent(record)?;

            records.push(Entity::from_inner(tracked));
        }

        Ok(TrackedSelectResult { result: records })
    }
}

pub struct Entity<T: Table> {
    inner: Rc<RefCell<T>>,
}

impl<T: Table> Entity<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn from_inner(inner: Rc<RefCell<T>>) -> Self {
        Self { inner }
    }

    pub fn read(&self) -> Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn write(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }

    pub(crate) fn inner(&self) -> &Rc<RefCell<T>> {
        &self.inner
    }
}

impl<T: Table> Clone for Entity<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T: Table> DeleteQueryRequest for DeleteQuery<'_, T> {
    fn table_name(&self) -> &'static str {
        T::table_name()
    }

    fn primary_key(&self) -> (&'static str, Value) {
        let fields = T::fields();
        let values = self.record.values();

        let pk = fields
            .iter()
            .find(|field| field.primary_key)
            .expect("primary key required");

        let (_, value) = values
            .into_iter()
            .find(|(name, _)| *name == pk.name)
            .expect("primary key value required");

        (pk.name, value)
    }
}

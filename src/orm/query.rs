use crate::{
    Connection, DbError, Value,
    orm::{Table, column::ColumnRef},
    query::ExecutableQuery,
};
use std::marker::PhantomData;

/// Represents a query for an ORM table.
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

pub(crate) trait OrmQueryRequest {
    fn table_name(&self) -> &'static str;
    fn fields(&self) -> Vec<ColumnRef>;
    fn where_clause(&self) -> &[Condition];
    fn order_by(&self) -> &[OrderBy];
    fn limit(&self) -> Option<usize>;
}

impl<T: Table> OrmQueryRequest for SelectQuery<T> {
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

impl<T: Table> ExecutableQuery for SelectQuery<T> {
    type Output = Vec<T>;

    fn execute(&self, connection: &mut Connection) -> Result<Self::Output, DbError> {
        let result = connection.execute_orm(self)?;

        result.rows().iter().map(T::from_row).collect()
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
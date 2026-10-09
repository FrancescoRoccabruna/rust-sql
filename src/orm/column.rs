use std::marker::PhantomData;

use crate::{
    Value, ValueType,
    orm::query::{Condition, OrderBy},
};

pub struct Nullable;
pub struct NotNullable;

/// Describes a column belonging to an ORM model.
#[derive(Debug, Clone, Copy)]
pub struct Column<T, N> {
    /// Database column name.
    pub name: &'static str,

    /// Rust/database value type.
    pub value_type: ValueType,

    /// Whether this column is the primary key.
    pub primary_key: bool,

    /// Whether this column accepts NULL.
    pub nullable: bool,

    _marker: PhantomData<(T, N)>,
}

impl<T, N> Column<T, N> {
    /// Creates an equality condition (`=`).
    pub fn eq<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::Eq(self.into(), value.into_column_value())
    }

    /// Creates an inequality condition (`!=`).
    pub fn ne<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::NotEq(self.into(), value.into_column_value())
    }

    /// Creates a greater-than condition (`>`).
    pub fn gt<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::Gt(self.into(), value.into_column_value())
    }

    /// Creates a greater-than-or-equal condition (`>=`).
    pub fn gte<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::Gte(self.into(), value.into_column_value())
    }

    /// Creates a less-than condition (`<`).
    pub fn lt<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::Lt(self.into(), value.into_column_value())
    }

    /// Creates a less-than-or-equal condition (`<=`).
    pub fn lte<V>(self, value: V) -> Condition
    where
        V: IntoColumnValue<T>,
    {
        Condition::Lte(self.into(), value.into_column_value())
    }

    /// Creates an ascending ordering expression.
    pub fn asc(self) -> OrderBy {
        OrderBy::Asc(self.into())
    }

    /// Creates a descending ordering expression.
    pub fn desc(self) -> OrderBy {
        OrderBy::Desc(self.into())
    }
}

impl<T> Column<T, Nullable> {
    /// Creates a condition that checks whether the column is NULL.
    pub fn is_null(self) -> Condition {
        Condition::IsNull(self.into())
    }

    /// Creates a condition that checks whether the column is not NULL.
    pub fn is_not_null(self) -> Condition {
        Condition::IsNotNull(self.into())
    }
}

impl<T, N> Column<T, N> {
    /// Creates a new ORM column.
    pub const fn new(
        name: &'static str,
        value_type: ValueType,
        primary_key: bool,
        nullable: bool,
    ) -> Self {
        Self {
            name,
            value_type,
            primary_key,
            nullable,
            _marker: PhantomData,
        }
    }
}

pub struct ColumnRef {
    pub name: &'static str,
    pub value_type: ValueType,
    pub primary_key: bool,
    pub nullable: bool,
}

impl<T, N> From<Column<T, N>> for ColumnRef {
    fn from(column: Column<T, N>) -> Self {
        Self {
            name: column.name,
            value_type: column.value_type,
            primary_key: column.primary_key,
            nullable: column.nullable,
        }
    }
}

impl ColumnRef {
    /// Creates a new column reference.
    pub const fn new(
        name: &'static str,
        value_type: ValueType,
        primary_key: bool,
        nullable: bool,
    ) -> Self {
        Self {
            name,
            value_type,
            primary_key,
            nullable,
        }
    }
}
pub struct ForeignKeyRef {
    pub columns: Vec<&'static str>,
    pub referenced_table: &'static str,
    pub referenced_columns: Vec<&'static str>,
}

pub trait IntoColumnValue<T> {
    fn into_column_value(self) -> Value;
}

impl IntoColumnValue<i64> for i64 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i64> for i32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i64> for i16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i64> for i8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u64> for u64 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u64> for u32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u64> for u16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u64> for u8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<f64> for f64 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<f64> for f32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u64> for usize {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i64> for isize {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<bool> for bool {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<String> for String {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<String> for &str {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i64> for u32 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<i64> for u16 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<i64> for u8 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<f64> for u32 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f64> for u16 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f64> for u8 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f64> for i32 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f64> for i16 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f64> for i8 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<Vec<u8>> for Vec<u8> {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<Vec<u8>> for &[u8] {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i32> for i32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i32> for i16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i32> for i8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u32> for u32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u32> for u16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u32> for u8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<f32> for f32 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i16> for i16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i16> for i8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u16> for u16 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u16> for u8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i8> for i8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<u8> for u8 {
    fn into_column_value(self) -> Value {
        self.into()
    }
}

impl IntoColumnValue<i32> for u16 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<i32> for u8 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<i16> for u8 {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

impl IntoColumnValue<f32> for u16 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f32> for i16 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f32> for u8 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<f32> for i8 {
    fn into_column_value(self) -> Value {
        Value::Float(self as f64)
    }
}

impl IntoColumnValue<usize> for usize {
    fn into_column_value(self) -> Value {
        Value::UInt(self as u64)
    }
}

impl IntoColumnValue<isize> for isize {
    fn into_column_value(self) -> Value {
        Value::Int(self as i64)
    }
}

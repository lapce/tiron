use std::ops::Range;

use hcl::{
    eval::{Context, Evaluate},
    Map, Number, Value,
};
use hcl_edit::{expr::Expression, Span};

use crate::error::{Error, Origin};

/// A wrapper type for attaching span information to a value.
#[derive(Debug, Clone, Eq)]
pub struct Spanned<T> {
    value: T,
    span: Option<Range<usize>>,
}

impl<T> PartialEq for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Spanned<T> {
    /// Creates a new `Spanned<T>` from a `T`.
    pub fn new(value: T) -> Spanned<T> {
        Spanned { value, span: None }
    }

    fn with_span(mut self, span: Option<Range<usize>>) -> Spanned<T> {
        self.span = span;
        self
    }

    /// Returns a reference to the wrapped value.
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn span(&self) -> &Option<Range<usize>> {
        &self.span
    }
}

/// Represents a value that is `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Null;

/// Represents any valid decorated HCL value.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SpannedValue {
    /// Represents a HCL null value.
    Null(Spanned<Null>),
    /// Represents a HCL boolean.
    Bool(Spanned<bool>),
    /// Represents a HCL number, either integer or float.
    Number(Spanned<Number>),
    /// Represents a HCL string.
    String(Spanned<String>),
    /// Represents a HCL array.
    Array(Spanned<Vec<SpannedValue>>),
    /// Represents a HCL object.
    Object(Spanned<Map<String, SpannedValue>>),
}

impl SpannedValue {
    pub fn span(&self) -> &Option<Range<usize>> {
        match self {
            SpannedValue::Null(v) => v.span(),
            SpannedValue::Bool(v) => v.span(),
            SpannedValue::Number(v) => v.span(),
            SpannedValue::String(v) => v.span(),
            SpannedValue::Array(v) => v.span(),
            SpannedValue::Object(v) => v.span(),
        }
    }

    pub fn from_value(value: Value, span: Option<Range<usize>>) -> SpannedValue {
        match value {
            Value::Null => SpannedValue::Null(Spanned::new(Null).with_span(span)),
            Value::Bool(bool) => SpannedValue::Bool(Spanned::new(bool).with_span(span)),
            Value::Number(v) => SpannedValue::Number(Spanned::new(v).with_span(span)),
            Value::String(v) => SpannedValue::String(Spanned::new(v).with_span(span)),
            Value::Array(array) => SpannedValue::Array(
                Spanned::new(
                    array
                        .into_iter()
                        .map(|v| SpannedValue::from_value(v, span.clone()))
                        .collect(),
                )
                .with_span(span),
            ),

            Value::Object(map) => SpannedValue::Object(
                Spanned::new(
                    map.into_iter()
                        .map(|(key, v)| (key, SpannedValue::from_value(v, span.clone())))
                        .collect(),
                )
                .with_span(span),
            ),
        }
    }

    pub fn from_expression(
        origin: &Origin,
        ctx: &Context,
        expr: hcl_edit::expr::Expression,
    ) -> Result<SpannedValue, Error> {
        let span = expr.span();
        match expr {
            Expression::Array(exprs) => {
                let mut values = Vec::new();
                for expr in exprs.into_iter() {
                    let value = SpannedValue::from_expression(origin, ctx, expr)?;
                    values.push(value);
                }
                Ok(SpannedValue::Array(Spanned::new(values).with_span(span)))
            }
            _ => {
                let expr: hcl::Expression = expr.into();
                let v: hcl::Value = expr
                    .evaluate(ctx)
                    .map_err(|e| origin.error(e.to_string(), &span))?;
                Ok(SpannedValue::from_value(v, span))
            }
        }
    }
}

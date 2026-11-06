//! Runtime support for props in Sycamore.

/// Builds a value of type `T` from a builder.
pub trait Build<T> {
    fn build(self) -> T;
}

#[diagnostic::on_unimplemented(
    message = "required field `{Field}` of type `{Value}` is missing",
    label = "set `{Field}` before calling `.build()`"
)]
pub trait FieldValue<Field, Value> {
    fn into_value(self) -> Value;
}

pub struct Missing;

pub struct Set<T>(pub(crate) T);

#[diagnostic::do_not_recommend]
impl<Field, T> FieldValue<Field, T> for Set<T> {
    fn into_value(self) -> T {
        self.0
    }
}

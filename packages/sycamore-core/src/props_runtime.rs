//! Runtime support for props in Sycamore.

/// Builds a value of type `T` from a builder.
///
/// Most commonly, the generic `T` is a `View` type and the trait is implemented for all types that
/// can be converted into a `View`. This would include things like `&str`, `String`, `View` itself,
/// and in particular, component builders.
pub trait Build<T> {
    /// Build the value of type `T`.
    fn build(self) -> T;
}

impl<T> Build<T> for T {
    fn build(self) -> T {
        self
    }
}

#[diagnostic::on_unimplemented(
    message = "required field `{Field}` of type `{Value}` is missing",
    label = "set `{Field}` before calling `.build()`"
)]
pub trait FieldValue<Field, Value> {
    fn into_value(self) -> Value;
}

pub struct Missing;

pub struct Set<T>(pub T);

#[diagnostic::do_not_recommend]
impl<Field, T> FieldValue<Field, T> for Set<T> {
    fn into_value(self) -> T {
        self.0
    }
}

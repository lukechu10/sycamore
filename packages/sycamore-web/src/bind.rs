//! Definition for bind-able attributes/properties.

use crate::events::EventDescriptor;
use crate::*;

/// Description for a bind-able attribute/property.
pub trait BindDescriptor {
    /// The event which we listen to update the value.
    type Event: EventDescriptor;
    /// The value of the signal that drives the attribute/property.
    type ValueTy: Into<JsValue> + Clone;
    /// The name of the property to which we are binding.
    const TARGET_PROPERTY: &'static str;
    /// Function for converting from JS to Rust type.
    const CONVERT_FROM_JS: for<'a> fn(&'a JsValue) -> Option<Self::ValueTy>;
}

macro_rules! impl_bind {
    ($name:ident: $event:ty, $value:ty, $target:expr, $fn:expr) => {
        #[allow(non_camel_case_types)]
        pub struct $name;
        impl BindDescriptor for $name {
            type Event = $event;
            type ValueTy = $value;
            const TARGET_PROPERTY: &'static str = $target;
            const CONVERT_FROM_JS: for<'a> fn(&'a JsValue) -> Option<Self::ValueTy> = $fn;
        }
    };
}

macro_rules! impl_binds {
    ($($name:ident: $event:ty, $value:ty, $target:expr, $fn:expr,)*) => {
        $(impl_bind!($name: $event, $value, $target, $fn);)*
    };
}

impl_binds! {
    value: events::input, String, "value", JsValue::as_string,
    valueAsNumber: events::input, f64, "valueAsNumber", JsValue::as_f64,
    checked: events::change, bool, "checked", JsValue::as_bool,
}

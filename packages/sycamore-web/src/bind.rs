//! Definition for bind-able attributes/properties.

use crate::*;

macro_rules! impl_bind {
    ($name:ident: $event_ty:ty, $value_ty:ty, $target:expr, $fn:expr) => {
        // #[allow(non_camel_case_types)]
        // pub struct $name;
        // impl BindDescriptor for $name {
        //     type Event = $event;
        //     type ValueTy = $value;
        //     const TARGET_PROPERTY: &'static str = $target;
        //     const CONVERT_FROM_JS: for<'a> fn(&'a JsValue) -> Option<Self::ValueTy> = $fn;
        // }
        paste::paste! {
            fn [<bind_ $name>](self, signal: Signal<$value_ty>) -> Self {
                self.bind_prop($target, signal, $fn)
            }
        }
    };
}

macro_rules! impl_binds {
    ($($name:ident: $event_ty:ty, $value_ty:ty, $target:expr, $fn:expr,)*) => {
        $(impl_bind!($name: $event_ty, $value_ty, $target, $fn);)*
    };
}

pub trait GlobalBinds: GlobalAttributes + Sized {
    // TODO: do not unwrap here. Instead print warning in console.
    impl_binds! {
        value: events::input, String, "value", |x| JsValue::as_string(x).unwrap(),
        value_as_number: events::input, f64, "valueAsNumber", |x| JsValue::as_f64(x).unwrap(),
        checked: events::change, bool, "checked", |x| JsValue::as_bool(x).unwrap(),
    }
}

impl<T: GlobalAttributes> GlobalBinds for T {}

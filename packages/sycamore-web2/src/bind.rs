//! Definition for bind-able attributes/properties.

use crate::*;

macro_rules! impl_bind {
    ($name:ident: $event:ident, $value_ty:ty, $prop:expr, $fn:expr) => {
        paste::paste! {
            fn [<bind_ $name>](self, signal: Signal<$value_ty>) -> Self {
                self.bind_prop(stringify!($event), $prop, signal, $fn)
            }
        }
    };
}

macro_rules! impl_binds {
    ($($name:ident: $event:ident, $value_ty:ty, $prop:expr, $fn:expr,)*) => {
        $(impl_bind!($name: $event, $value_ty, $prop, $fn);)*
    };
}

pub trait GlobalBinds: GlobalAttributes + Sized {
    // TODO: do not unwrap here. Instead print warning in console.
    impl_binds! {
        value: input, String, "value", |x| JsValue::as_string(x).unwrap(),
        value_as_number: input, f64, "valueAsNumber", |x| JsValue::as_f64(x).unwrap(),
        checked: change, bool, "checked", |x| JsValue::as_bool(x).unwrap(),
    }
}

impl<T: GlobalAttributes> GlobalBinds for T {}

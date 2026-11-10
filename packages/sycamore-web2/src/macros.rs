/// Log a message to the JavaScript console if on wasm32. Otherwise logs it to stdout.
///
/// Note: this does not work properly for server-side WASM since it will mistakenly try to log to
/// the JS console.
#[macro_export]
macro_rules! console_log {
    ($($arg:tt)*) => {
        if $crate::is_not_ssr!() {
            $crate::rt::web_sys::console::log_1(&::std::format!($($arg)*).into());
        } else {
            ::std::println!($($arg)*);
        }
    };
}

/// Log a warning to the JavaScript console if on wasm32. Otherwise logs it to stderr.
///
/// Note: this does not work properly for server-side WASM since it will mistakenly try to log to
/// the JS console.
#[macro_export]
macro_rules! console_warn {
    ($($arg:tt)*) => {
        if $crate::is_not_ssr!() {
            $crate::rt::web_sys::console::warn_1(&::std::format!($($arg)*).into());
        } else {
            ::std::eprintln!($($arg)*);
        }
    };
}

/// Prints an error message to the JavaScript console if on wasm32. Otherwise logs it to stderr.
///
/// Note: this does not work properly for server-side WASM since it will mistakenly try to log to
/// the JS console.
#[macro_export]
macro_rules! console_error {
    ($($arg:tt)*) => {
        if $crate::is_not_ssr!() {
            $crate::rt::web_sys::console::error_1(&::std::format!($($arg)*).into());
        } else {
            ::std::eprintln!($($arg)*);
        }
    };
}

/// Debug the value of a variable to the JavaScript console if on wasm32. Otherwise logs it to
/// stdout.
///
/// Note: this does not work properly for server-side WASM since it will mistakenly try to log to
/// the JS console.
#[macro_export]
macro_rules! console_dbg {
    () => {
        if $crate::is_not_ssr!() {
            $crate::rt::web_sys::console::log_1(
                &::std::format!("[{}:{}]", ::std::file!(), ::std::line!(),).into(),
            );
        } else {
            ::std::dbg!($arg);
        }
    };
    ($arg:expr $(,)?) => {
        if $crate::is_not_ssr!() {
            let arg = $arg;
            $crate::rt::web_sys::console::log_1(
                &::std::format!(
                    "[{}:{}] {} = {:#?}",
                    ::std::file!(),
                    ::std::line!(),
                    ::std::stringify!($arg),
                    arg
                )
                .into(),
            );
            arg
        } else {
            ::std::dbg!($arg)
        }
    };
    ($($arg:expr),+ $(,)?) => {
        $($crate::console_dbg!($arg);)+
    }
}

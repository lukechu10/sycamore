//! Utilities for smooth transitions and animations.

use std::cell::OnceCell;
use std::rc::Rc;

use crate::reactive::*;

/// Type returned by [`create_raf`] and [`create_raf_loop`].
type RafState = (Signal<bool>, Rc<dyn Fn() + 'static>, Rc<dyn Fn() + 'static>);

/// Schedule a callback to be called on each animation frame.
/// Does nothing if not on `wasm32` target.
///
/// Returns a tuple of `(running, start, stop)`. The first item is a boolean signal representing
/// whether the raf is currently running. The second item is a function to start the raf. The
/// third item is a function to stop the raf.
///
/// The raf is not started by default. Call the `start` function to initiate the raf.
pub fn create_raf(mut cb: impl FnMut() + 'static) -> RafState {
    let running = create_signal(false);
    let start: Rc<dyn Fn()>;
    let stop: Rc<dyn Fn()>;
    let _ = &mut cb;

    // Only run on wasm32 architecture.
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    {
        use std::cell::RefCell;

        use wasm_bindgen::prelude::*;

        use crate::web::window;

        let f = Rc::new(RefCell::new(None::<Closure<dyn FnMut()>>));
        let g = Rc::clone(&f);

        *g.borrow_mut() = Some(Closure::new(move || {
            if running.get() {
                // Verified that scope is still valid. We can access `extended` in here.
                cb();
                // Request the next raf frame.
                window()
                    .request_animation_frame(
                        f.borrow().as_ref().unwrap_throw().as_ref().unchecked_ref(),
                    )
                    .unwrap_throw();
            }
        }));
        start = Rc::new(move || {
            if !running.get() {
                running.set(true);
                window()
                    .request_animation_frame(
                        g.borrow().as_ref().unwrap_throw().as_ref().unchecked_ref(),
                    )
                    .unwrap_throw();
            }
        });
        stop = Rc::new(move || running.set(false));
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "web")))]
    {
        start = Rc::new(move || running.set(true));
        stop = Rc::new(move || running.set(false));
    }

    (running, start, stop)
}

/// Schedule a callback to be called on each animation frame.
/// Does nothing if not on `wasm32` target.
///
/// Instead of using `start` and `stop` functions, the callback is kept on looping until it
/// returns `false`. `start` and `stop` are returned regardless to allow controlling the
/// looping from outside the function.
///
/// The raf is not started by default. Call the `start` function to initiate the raf.
pub fn create_raf_loop(mut f: impl FnMut() -> bool + 'static) -> RafState {
    let stop_shared = Rc::new(OnceCell::<Rc<dyn Fn()>>::new());
    let (running, start, stop) = create_raf({
        let stop_shared = Rc::clone(&stop_shared);
        move || {
            if !f() {
                stop_shared.get().unwrap()();
            }
        }
    });
    stop_shared.set(Rc::clone(&stop)).ok().unwrap();
    (running, start, stop)
}

/// Create a new [`Tweened`] signal.
pub fn create_tweened_signal<T: Lerp + Clone>(
    initial: T,
    transition_duration: std::time::Duration,
    easing_fn: impl Fn(f32) -> f32 + 'static,
) -> Tweened<T> {
    Tweened::new(initial, transition_duration, easing_fn)
}

/// Describes a trait that can be linearly interpolate between two points.
pub trait Lerp {
    /// Get a value between `cx` and `other` at a `scalar`.
    ///
    /// `0.0 <= scalar <= 1`
    fn lerp(&self, other: &Self, scalar: f32) -> Self;
}

macro_rules! impl_lerp_for_float {
    ($($f: path),*) => {
        $(
            impl Lerp for $f {
                fn lerp(&self, other: &Self, scalar: f32) -> Self {
                    self + (other - self) * scalar as $f
                }
            }
        )*
    };
}

impl_lerp_for_float!(f32, f64);

macro_rules! impl_lerp_for_int {
    ($($i: path),*) => {
        $(
            impl Lerp for $i {
                fn lerp(&self, other: &Self, scalar: f32) -> Self {
                    (*self as f32 + (other - self) as f32 * scalar).round() as $i
                }
            }
        )*
    };
}

impl_lerp_for_int!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

impl<T: Lerp + Clone, const N: usize> Lerp for [T; N] {
    fn lerp(&self, other: &Self, scalar: f32) -> Self {
        let mut tmp = (*self).clone();

        for (t, other) in tmp.iter_mut().zip(other) {
            *t = t.lerp(other, scalar);
        }

        tmp
    }
}

/// A state that is interpolated when it is set.
pub struct Tweened<T: Lerp + Clone + 'static>(Signal<TweenedInner<T>>);
impl<T: Lerp + Clone> std::fmt::Debug for Tweened<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tweened").finish()
    }
}

struct TweenedInner<T: Lerp + Clone + 'static> {
    value: Signal<T>,
    is_tweening: Signal<bool>,
    raf_state: Option<RafState>,
    transition_duration_ms: f32,
    easing_fn: Rc<dyn Fn(f32) -> f32>,
}

impl<T: Lerp + Clone> Tweened<T> {
    /// Create a new tweened state with the given value.
    ///
    /// End users should use [`Scope::create_tweened_signal`] instead.
    pub(crate) fn new(
        initial: T,
        transition_duration: std::time::Duration,
        easing_fn: impl Fn(f32) -> f32 + 'static,
    ) -> Self {
        let value = create_signal(initial);
        Self(create_signal(TweenedInner {
            value,
            is_tweening: create_signal(false),
            raf_state: None,
            transition_duration_ms: transition_duration.as_millis() as f32,
            easing_fn: Rc::new(easing_fn),
        }))
    }

    /// Set the target value for the `Tweened`. The existing value will be interpolated to the
    /// target value with the specified `transition_duration` and `easing_fn`.
    ///
    /// If the value is being interpolated already due to a previous call to `set()`, the previous
    /// task will be canceled.
    ///
    /// To immediately set the value without interpolating the value, use `signal().set(...)`
    /// instead.
    ///
    /// If not running on `wasm32-unknown-unknown`, does nothing.
    pub fn set(&self, _new_value: T) {
        #[cfg(all(target_arch = "wasm32", feature = "web"))]
        {
            use web_sys::js_sys::Date;

            let start = self.signal().get_clone_untracked();
            let easing_fn = Rc::clone(&self.0.with(|this| this.easing_fn.clone()));

            let start_time = Date::now();
            let signal = self.0.with(|this| this.value.clone());
            let is_tweening = self.0.with(|this| this.is_tweening.clone());
            let transition_duration_ms = self.0.with(|this| this.transition_duration_ms);

            // If previous raf is still running, call stop() to cancel it.
            if let Some((running, _, stop)) = &self.0.with(|this| this.raf_state.clone()) {
                if running.get_untracked() {
                    stop();
                }
            }

            let (running, start, stop) = create_raf_loop(move || {
                let now = Date::now();

                let since_start = now - start_time;
                let scalar = since_start as f32 / transition_duration_ms;

                if now < start_time + transition_duration_ms as f64 {
                    signal.set(start.lerp(&_new_value, easing_fn(scalar)));
                    true
                } else {
                    signal.set(_new_value.clone());
                    is_tweening.set(false);
                    false
                }
            });
            start();
            is_tweening.set(true);
            self.0
                .update(|this| this.raf_state = Some((running, start, stop)));
        }
    }

    /// Alias for `signal().get()`.
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        self.signal().get()
    }

    /// Alias for `signal().get_untracked()`.
    pub fn get_untracked(&self) -> T
    where
        T: Copy,
    {
        self.signal().get_untracked()
    }

    /// Get the inner signal backing the state.
    pub fn signal(&self) -> Signal<T> {
        self.0.with(|this| this.value)
    }

    /// Returns `true` if the value is currently being tweened/interpolated. This value is reactive
    /// and can be tracked.
    pub fn is_tweening(&self) -> bool {
        self.0.with(|this| this.is_tweening.get())
    }
}

impl<T: Lerp + Clone + 'static> Clone for Tweened<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Lerp + Clone + 'static> Copy for Tweened<T> {}

impl<T: Lerp + Clone + 'static> Clone for TweenedInner<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            is_tweening: self.is_tweening,
            raf_state: self.raf_state.clone(),
            transition_duration_ms: self.transition_duration_ms,
            easing_fn: Rc::clone(&self.easing_fn),
        }
    }
}

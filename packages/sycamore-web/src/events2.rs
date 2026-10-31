//! Definition for all the events that can be listened to.

use wasm_bindgen::JsCast;
pub use web_sys::{
    AnimationEvent, BeforeUnloadEvent, CompositionEvent, DeviceMotionEvent, DeviceOrientationEvent,
    DragEvent, ErrorEvent, Event, FocusEvent, GamepadEvent, HashChangeEvent, InputEvent,
    KeyboardEvent, MessageEvent, MouseEvent, PageTransitionEvent, PointerEvent, PopStateEvent,
    ProgressEvent, PromiseRejectionEvent, SecurityPolicyViolationEvent, StorageEvent, SubmitEvent,
    TouchEvent, TransitionEvent, UiEvent, WheelEvent,
};

use crate::GlobalAttributes;

/// A marker struct for synchronous event handlers.
struct SyncHandler;
/// A marker struct for asynchronous event handlers.
struct AsyncHandler;

/// A wrapper around an event handler that can be either synchronous or asynchronous.
/// This struct is intended to be used as an attribute type that can automatically converted from a
/// closure (that is possibly async) into an event handler that can be attached to an element.
pub struct EventHandler<T: AsRef<Event>, Kind = ()> {
    callback: Box<dyn FnMut(T) + 'static>,
    _phantom: std::marker::PhantomData<(T, Kind)>,
}

impl<T: AsRef<Event>, Kind> EventHandler<T, Kind> {
    /// Call the event handler with the given event.
    pub fn call(&mut self, event: T) {
        (self.callback)(event);
    }
}

impl<F, T> From<F> for EventHandler<T, SyncHandler>
where
    F: FnMut(T) + 'static,
    T: AsRef<Event> + 'static,
{
    fn from(f: F) -> Self {
        Self {
            callback: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "suspense")]
impl<F, Fut, T> From<F> for EventHandler<T, AsyncHandler>
where
    F: FnMut(T) -> Fut + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
    T: AsRef<Event> + 'static,
{
    fn from(mut f: F) -> Self {
        Self {
            callback: Box::new(move |event| {
                let fut = f(event);
                sycamore_futures::spawn_local_scoped(fut);
            }),
            _phantom: std::marker::PhantomData,
        }
    }
}

macro_rules! impl_event {
    ($name:ident: $ty:ty) => {
        paste::paste! {
            #[allow(non_snake_case)]
            fn [<on_ $name>]<F, Kind>(self, handler: F) -> Self
            where
                F: Into<EventHandler<$ty, Kind>> + 'static,
                Kind: 'static,
            {
                let mut handler = handler.into();
                self.add_event_handler(stringify!($name), move |ev: web_sys::Event| handler.call(ev.unchecked_into()))
            }
        }
    };
}

macro_rules! impl_events {
    ($($name:ident: $ty:ty,)*) => {
        $(impl_event!($name: $ty);)*
    };
}

pub trait GlobalEvents: GlobalAttributes + Sized {
    impl_events! {
        afterprint: Event,
        beforeprint: Event,
        beforeunload: BeforeUnloadEvent,
        gamepadconnected: GamepadEvent,
        gamepaddisconnected: GamepadEvent,
        hashchange: HashChangeEvent,
        languagechange: Event,
        message: MessageEvent,
        messageerror: MessageEvent,
        offline: Event,
        online: Event,
        pagehide: PageTransitionEvent,
        pageshow: PageTransitionEvent,
        popstate: PopStateEvent,
        rejectionhandled: PromiseRejectionEvent,
        storage: StorageEvent,
        unhandledrejection: PromiseRejectionEvent,
        unload: Event,

        /*
        GlobalEventHandlersEventMap
        */
        abort: UiEvent,
        animationcancel: AnimationEvent,
        animationend: AnimationEvent,
        animationiteration: AnimationEvent,
        animationstart: AnimationEvent,
        auxclick: MouseEvent,
        beforeinput: InputEvent,
        blur: FocusEvent,
        canplay: Event,
        canplaythrough: Event,
        change: Event,
        click: MouseEvent,
        close: Event,
        compositionend: CompositionEvent,
        compositionstart: CompositionEvent,
        compositionupdate: CompositionEvent,
        contextmenu: MouseEvent,
        cuechange: Event,
        dblclick: MouseEvent,
        drag: DragEvent,
        dragend: DragEvent,
        dragenter: DragEvent,
        dragleave: DragEvent,
        dragover: DragEvent,
        dragstart: DragEvent,
        drop: DragEvent,
        durationchange: Event,
        emptied: Event,
        ended: Event,
        error: ErrorEvent,
        focus: FocusEvent,
        focusin: FocusEvent,
        focusout: FocusEvent,
        formdata: Event, // web_sys does not include `FormDataEvent`
        gotpointercapture: PointerEvent,
        input: Event,
        invalid: Event,
        keydown: KeyboardEvent,
        keypress: KeyboardEvent,
        keyup: KeyboardEvent,
        load: Event,
        loadeddata: Event,
        loadedmetadata: Event,
        loadstart: Event,
        lostpointercapture: PointerEvent,
        mousedown: MouseEvent,
        mouseenter: MouseEvent,
        mouseleave: MouseEvent,
        mousemove: MouseEvent,
        mouseout: MouseEvent,
        mouseover: MouseEvent,
        mouseup: MouseEvent,
        pause: Event,
        play: Event,
        playing: Event,
        pointercancel: PointerEvent,
        pointerdown: PointerEvent,
        pointerenter: PointerEvent,
        pointerleave: PointerEvent,
        pointermove: PointerEvent,
        pointerout: PointerEvent,
        pointerover: PointerEvent,
        pointerup: PointerEvent,
        progress: ProgressEvent,
        ratechange: Event,
        reset: Event,
        resize: UiEvent,
        scroll: Event,
        securitypolicyviolation: SecurityPolicyViolationEvent,
        seeked: Event,
        seeking: Event,
        select: Event,
        selectionchange: Event,
        selectstart: Event,
        slotchange: Event,
        stalled: Event,
        submit: SubmitEvent,
        suspend: Event,
        timeupdate: Event,
        toggle: Event,
        touchcancel: TouchEvent,
        touchend: TouchEvent,
        touchmove: TouchEvent,
        touchstart: TouchEvent,
        transitioncancel: TransitionEvent,
        transitionend: TransitionEvent,
        transitionrun: TransitionEvent,
        transitionstart: TransitionEvent,
        volumechange: Event,
        waiting: Event,
        webkitanimationend: Event,
        webkitanimationiteration: Event,
        webkitanimationstart: Event,
        webkittransitionend: Event,
        wheel: WheelEvent,

        /*
        WindowEventMap
        */
        DOMContentLoaded: Event,
        devicemotion: DeviceMotionEvent,
        deviceorientation: DeviceOrientationEvent,
        orientationchange: Event,

        /*
        DocumentAndElementEventHandlersEventMap
        */
        copy: Event, // ClipboardEvent is unstable
        cut: Event, // ClipboardEvent is unstable
        paste: Event, // ClipboardEvent is unstable

        /*
        DocumentEventMap
        */
        fullscreenchange: Event,
        fullscreenerror: Event,
        pointerlockchange: Event,
        pointerlockerror: Event,
        readystatechange: Event,
        visibilitychange: Event,
    }
}

impl<T: GlobalAttributes> GlobalEvents for T {}

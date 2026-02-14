use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// Accumulated input state read each frame by the application.
pub struct InputState {
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    pub scroll_delta: f32,
    pub left_button_down: bool,
    pub middle_button_down: bool,
    pub right_button_down: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            scroll_delta: 0.0,
            left_button_down: false,
            middle_button_down: false,
            right_button_down: false,
        }
    }

    /// Clear per-frame deltas (called after camera update consumes them).
    pub fn clear_deltas(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.scroll_delta = 0.0;
    }
}

/// Register mouse/wheel event listeners on the canvas ONCE at init (C-RUST-3).
/// Closures are leaked via `.forget()` since they live for the app lifetime.
pub fn register_input_listeners(
    canvas: &web_sys::HtmlCanvasElement,
    state: Rc<RefCell<InputState>>,
) {
    let target: &web_sys::EventTarget = canvas.as_ref();

    // mousemove
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let mut s = state.borrow_mut();
                s.mouse_dx += e.movement_x() as f32;
                s.mouse_dy += e.movement_y() as f32;
            });
        target
            .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
            .expect("failed to add mousemove listener");
        closure.forget();
    }

    // mousedown
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let mut s = state.borrow_mut();
                match e.button() {
                    0 => s.left_button_down = true,
                    1 => s.middle_button_down = true,
                    2 => s.right_button_down = true,
                    _ => {}
                }
            });
        target
            .add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())
            .expect("failed to add mousedown listener");
        closure.forget();
    }

    // mouseup
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                let mut s = state.borrow_mut();
                match e.button() {
                    0 => s.left_button_down = false,
                    1 => s.middle_button_down = false,
                    2 => s.right_button_down = false,
                    _ => {}
                }
            });
        target
            .add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())
            .expect("failed to add mouseup listener");
        closure.forget();
    }

    // wheel (scroll zoom)
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::WheelEvent)>::new(move |e: web_sys::WheelEvent| {
                e.prevent_default();
                let mut s = state.borrow_mut();
                // Normalize delta: deltaY is positive for scroll down
                let delta = -e.delta_y() as f32;
                // Normalize for different deltaMode values
                s.scroll_delta += if e.delta_mode() == 1 {
                    delta * 20.0 // line mode
                } else {
                    delta / 3.0 // pixel mode
                };
            });
        // Use non-passive listener so preventDefault works
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(false);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                closure.as_ref().unchecked_ref(),
                &options,
            )
            .expect("failed to add wheel listener");
        closure.forget();
    }

    // contextmenu (prevent right-click menu)
    {
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                e.prevent_default();
            });
        target
            .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
            .expect("failed to add contextmenu listener");
        closure.forget();
    }
}

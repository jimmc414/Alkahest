use std::cell::RefCell;
use std::collections::HashSet;
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
    /// Currently held keys.
    pub keys_down: HashSet<String>,
    /// Keys pressed this frame (consumed on read).
    pub keys_just_pressed: Vec<String>,
    /// Whether shift is held.
    pub shift_down: bool,
    /// Mouse position in CSS pixels.
    pub mouse_x: f32,
    pub mouse_y: f32,
    /// Whether the pointer is currently locked (first-person mode).
    pub pointer_locked: bool,
    /// Set to true for one frame when pointer lock is lost (Escape pressed).
    pub pointer_lock_lost: bool,
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
            keys_down: HashSet::new(),
            keys_just_pressed: Vec::new(),
            shift_down: false,
            mouse_x: 0.0,
            mouse_y: 0.0,
            pointer_locked: false,
            pointer_lock_lost: false,
        }
    }

    /// Check if a key was just pressed this frame.
    pub fn was_just_pressed(&self, key: &str) -> bool {
        self.keys_just_pressed.iter().any(|k| k == key)
    }

    /// Clear per-frame deltas (called after camera update consumes them).
    pub fn clear_deltas(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.scroll_delta = 0.0;
        self.keys_just_pressed.clear();
        self.pointer_lock_lost = false;
    }
}

/// Register mouse/wheel/keyboard event listeners on the canvas ONCE at init (C-RUST-3).
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
                s.mouse_x = e.offset_x() as f32;
                s.mouse_y = e.offset_y() as f32;
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

    // Keyboard events: register on the window (canvas doesn't receive key events by default)
    let window: web_sys::EventTarget = web_sys::window().expect("no global window").into();

    // keydown
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                let mut s = state.borrow_mut();
                let key = e.key();
                s.shift_down = e.shift_key();
                if !s.keys_down.contains(&key) {
                    s.keys_just_pressed.push(key.clone());
                }
                s.keys_down.insert(key);
            });
        window
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .expect("failed to add keydown listener");
        closure.forget();
    }

    // keyup
    {
        let state = state.clone();
        let closure =
            Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                let mut s = state.borrow_mut();
                s.shift_down = e.shift_key();
                s.keys_down.remove(&e.key());
            });
        window
            .add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())
            .expect("failed to add keyup listener");
        closure.forget();
    }

    // pointerlockchange — track pointer lock state
    {
        let state = state.clone();
        let doc: web_sys::EventTarget = web_sys::window()
            .expect("no global window")
            .document()
            .expect("no document")
            .into();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let document = web_sys::window()
                .expect("no global window")
                .document()
                .expect("no document");
            let locked = document.pointer_lock_element().is_some();
            let mut s = state.borrow_mut();
            if s.pointer_locked && !locked {
                s.pointer_lock_lost = true;
            }
            s.pointer_locked = locked;
        });
        doc.add_event_listener_with_callback("pointerlockchange", closure.as_ref().unchecked_ref())
            .expect("failed to add pointerlockchange listener");
        closure.forget();
    }
}

/// Request pointer lock on the canvas element (for first-person camera).
pub fn request_pointer_lock() {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Some(canvas) = document.get_element_by_id("alkahest-canvas") {
            canvas.request_pointer_lock();
        }
    }
}

/// Release pointer lock (back to orbit camera).
pub fn exit_pointer_lock() {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        document.exit_pointer_lock();
    }
}

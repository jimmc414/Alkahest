mod app;
mod camera;
mod commands;
mod gpu;
mod input;
mod tools;
pub mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// WASM entry point. Sets panic hook (C-RUST-5) and initializes logging.
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("logger init failed");
    log::info!("Alkahest starting...");

    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            show_error_to_user(&format!("{e}"));
        }
    });
}

/// Async initialization: gets canvas, computes physical size, creates Application.
async fn run() -> Result<(), alkahest_core::error::AlkahestError> {
    let window = web_sys::window().expect("no global window");
    let document = window.document().expect("no document");

    let canvas = document
        .get_element_by_id("alkahest-canvas")
        .expect("canvas element not found")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("element is not a canvas");

    // Compute physical pixel size from DPI (C-EGUI-3)
    let dpi_scale = window.device_pixel_ratio() as f32;
    let css_width = canvas.client_width() as f32;
    let css_height = canvas.client_height() as f32;
    let physical_width = (css_width * dpi_scale) as u32;
    let physical_height = (css_height * dpi_scale) as u32;

    // Set canvas backing store to physical pixels
    canvas.set_width(physical_width);
    canvas.set_height(physical_height);

    log::info!(
        "Canvas: {}x{} CSS, {}x{} physical (DPI: {:.2})",
        css_width,
        css_height,
        physical_width,
        physical_height,
        dpi_scale
    );

    // Register input listeners on canvas ONCE (C-RUST-3)
    let input_state = Rc::new(RefCell::new(input::InputState::new()));
    input::register_input_listeners(&canvas, input_state.clone());

    let gpu_ctx = gpu::init_gpu(canvas, physical_width, physical_height).await?;
    let application = app::Application::new(gpu_ctx, dpi_scale, input_state);
    let app_rc = Rc::new(RefCell::new(application));

    app::Application::start_loop(app_rc);

    Ok(())
}

/// Show a user-visible error (C-GPU-1: not just console).
fn show_error_to_user(msg: &str) {
    log::error!("{msg}");
    let window = web_sys::window().expect("no global window");
    let _ = window.alert_with_message(&format!("Alkahest Error: {msg}"));
}

use wasm_bindgen::prelude::*;

/// Check if the File System Access API (showSaveFilePicker) is available.
pub fn has_file_system_access() -> bool {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return false,
    };
    js_sys::Reflect::has(&window, &JsValue::from_str("showSaveFilePicker")).unwrap_or(false)
}

/// Save data to a file. Uses File System Access API if available, otherwise Blob URL fallback.
pub fn save_to_file(data: &[u8], filename: &str) {
    if has_file_system_access() {
        save_via_file_system_access(data, filename);
    } else {
        save_via_blob_url(data, filename);
    }
}

/// Save using the File System Access API (Chrome/Edge).
fn save_via_file_system_access(data: &[u8], filename: &str) {
    let array = js_sys::Uint8Array::from(data);
    let filename = filename.to_string();

    wasm_bindgen_futures::spawn_local(async move {
        match save_file_picker_inner(&array, &filename).await {
            Ok(()) => log::info!("File saved via File System Access API"),
            Err(e) => {
                // User cancelled or API error — fall back to blob
                log::warn!(
                    "File System Access API failed ({:?}), falling back to blob",
                    e
                );
                save_blob_from_array(&array, &filename);
            }
        }
    });
}

async fn save_file_picker_inner(array: &js_sys::Uint8Array, filename: &str) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no global window");

    // Build options: { suggestedName: filename, types: [{ accept: { 'application/octet-stream': ['.alka'] } }] }
    let accept = js_sys::Object::new();
    let extensions = js_sys::Array::new();
    extensions.push(&JsValue::from_str(".alka"));
    js_sys::Reflect::set(
        &accept,
        &JsValue::from_str("application/octet-stream"),
        &extensions,
    )?;

    let file_type = js_sys::Object::new();
    js_sys::Reflect::set(&file_type, &JsValue::from_str("accept"), &accept)?;

    let types = js_sys::Array::new();
    types.push(&file_type);

    let options = js_sys::Object::new();
    js_sys::Reflect::set(
        &options,
        &JsValue::from_str("suggestedName"),
        &JsValue::from_str(filename),
    )?;
    js_sys::Reflect::set(&options, &JsValue::from_str("types"), &types)?;

    // Call showSaveFilePicker(options)
    let picker_fn = js_sys::Reflect::get(&window, &JsValue::from_str("showSaveFilePicker"))?;
    let picker_fn: js_sys::Function = picker_fn.dyn_into()?;
    let handle_promise: js_sys::Promise = picker_fn.call1(&window, &options)?.dyn_into()?;
    let handle = wasm_bindgen_futures::JsFuture::from(handle_promise).await?;

    // Call handle.createWritable()
    let create_writable = js_sys::Reflect::get(&handle, &JsValue::from_str("createWritable"))?;
    let create_writable: js_sys::Function = create_writable.dyn_into()?;
    let writable_promise: js_sys::Promise = create_writable.call0(&handle)?.dyn_into()?;
    let writable = wasm_bindgen_futures::JsFuture::from(writable_promise).await?;

    // Call writable.write(data)
    let write_fn = js_sys::Reflect::get(&writable, &JsValue::from_str("write"))?;
    let write_fn: js_sys::Function = write_fn.dyn_into()?;
    let write_promise: js_sys::Promise = write_fn.call1(&writable, array)?.dyn_into()?;
    wasm_bindgen_futures::JsFuture::from(write_promise).await?;

    // Call writable.close()
    let close_fn = js_sys::Reflect::get(&writable, &JsValue::from_str("close"))?;
    let close_fn: js_sys::Function = close_fn.dyn_into()?;
    let close_promise: js_sys::Promise = close_fn.call0(&writable)?.dyn_into()?;
    wasm_bindgen_futures::JsFuture::from(close_promise).await?;

    Ok(())
}

/// Save using a Blob URL + <a download> fallback (Firefox/Safari).
fn save_via_blob_url(data: &[u8], filename: &str) {
    let array = js_sys::Uint8Array::from(data);
    save_blob_from_array(&array, filename);
}

fn save_blob_from_array(array: &js_sys::Uint8Array, filename: &str) {
    let parts = js_sys::Array::new();
    parts.push(array);

    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .expect("blob creation failed");

    let url = web_sys::Url::create_object_url_with_blob(&blob).expect("create_object_url failed");

    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .expect("create_element failed")
        .dyn_into()
        .expect("not an anchor");

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.style().set_property("display", "none").ok();

    let body = document.body().expect("no body");
    body.append_child(&anchor).ok();
    anchor.click();
    body.remove_child(&anchor).ok();

    web_sys::Url::revoke_object_url(&url).ok();
    log::info!("File saved via Blob URL download");
}

/// Trigger a file open dialog and call the callback with the loaded bytes.
/// Uses File System Access API if available, otherwise <input type="file"> fallback.
pub fn load_from_file(callback: impl FnOnce(Vec<u8>) + 'static) {
    if has_file_system_access() {
        load_via_file_system_access(callback);
    } else {
        load_via_input_element(callback);
    }
}

/// Load using the File System Access API (Chrome/Edge).
fn load_via_file_system_access(callback: impl FnOnce(Vec<u8>) + 'static) {
    wasm_bindgen_futures::spawn_local(async move {
        match load_file_picker_inner().await {
            Ok(bytes) => callback(bytes),
            Err(e) => {
                log::warn!(
                    "File System Access load failed ({:?}), trying input fallback",
                    e
                );
                load_via_input_element(callback);
            }
        }
    });
}

async fn load_file_picker_inner() -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().expect("no global window");

    // Build options: { types: [{ accept: { 'application/octet-stream': ['.alka'] } }] }
    let accept = js_sys::Object::new();
    let extensions = js_sys::Array::new();
    extensions.push(&JsValue::from_str(".alka"));
    js_sys::Reflect::set(
        &accept,
        &JsValue::from_str("application/octet-stream"),
        &extensions,
    )?;

    let file_type = js_sys::Object::new();
    js_sys::Reflect::set(&file_type, &JsValue::from_str("accept"), &accept)?;

    let types = js_sys::Array::new();
    types.push(&file_type);

    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &JsValue::from_str("types"), &types)?;

    let picker_fn = js_sys::Reflect::get(&window, &JsValue::from_str("showOpenFilePicker"))?;
    let picker_fn: js_sys::Function = picker_fn.dyn_into()?;
    let handles_promise: js_sys::Promise = picker_fn.call1(&window, &options)?.dyn_into()?;
    let handles = wasm_bindgen_futures::JsFuture::from(handles_promise).await?;
    let handles: js_sys::Array = handles.dyn_into()?;

    if handles.length() == 0 {
        return Err(JsValue::from_str("no file selected"));
    }

    let handle = handles.get(0);
    let get_file = js_sys::Reflect::get(&handle, &JsValue::from_str("getFile"))?;
    let get_file: js_sys::Function = get_file.dyn_into()?;
    let file_promise: js_sys::Promise = get_file.call0(&handle)?.dyn_into()?;
    let file = wasm_bindgen_futures::JsFuture::from(file_promise).await?;

    let array_buffer_fn = js_sys::Reflect::get(&file, &JsValue::from_str("arrayBuffer"))?;
    let array_buffer_fn: js_sys::Function = array_buffer_fn.dyn_into()?;
    let buffer_promise: js_sys::Promise = array_buffer_fn.call0(&file)?.dyn_into()?;
    let buffer = wasm_bindgen_futures::JsFuture::from(buffer_promise).await?;
    let array = js_sys::Uint8Array::new(&buffer);

    Ok(array.to_vec())
}

/// Load using an <input type="file"> element (Firefox/Safari fallback).
fn load_via_input_element(callback: impl FnOnce(Vec<u8>) + 'static) {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");
    let input: web_sys::HtmlInputElement = document
        .create_element("input")
        .expect("create_element failed")
        .dyn_into()
        .expect("not an input");

    input.set_type("file");
    input.set_accept(".alka");
    input.style().set_property("display", "none").ok();

    let body = document.body().expect("no body");
    body.append_child(&input).ok();

    let input_clone = input.clone();
    let callback = std::cell::RefCell::new(Some(callback));

    let closure = Closure::<dyn FnMut()>::new(move || {
        let files = match input_clone.files() {
            Some(f) => f,
            None => return,
        };
        if files.length() == 0 {
            return;
        }
        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let reader = web_sys::FileReader::new().expect("FileReader creation failed");
        let reader_clone = reader.clone();
        let cb = std::cell::RefCell::new(callback.borrow_mut().take());

        let onload = Closure::<dyn FnMut()>::new(move || {
            if let Some(cb) = cb.borrow_mut().take() {
                let result = reader_clone.result().expect("FileReader result missing");
                let buffer = js_sys::ArrayBuffer::from(result);
                let array = js_sys::Uint8Array::new(&buffer);
                cb(array.to_vec());
            }
        });

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget(); // Lives for the callback's lifetime

        reader
            .read_as_array_buffer(&file)
            .expect("read_as_array_buffer failed");

        // Clean up the input element
        if let Some(parent) = input_clone.parent_node() {
            parent.remove_child(&input_clone).ok();
        }
    });

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget(); // Lives for the click callback's lifetime

    input.click();
}

/// Save data to IndexedDB for auto-save.
pub fn auto_save_to_idb(data: &[u8], slot: &str) {
    let array = js_sys::Uint8Array::from(data);
    let slot = slot.to_string();

    wasm_bindgen_futures::spawn_local(async move {
        match idb_put(&slot, &array).await {
            Ok(()) => log::info!("Auto-save written to IndexedDB slot '{}'", slot),
            Err(e) => log::warn!("Auto-save to IndexedDB failed: {:?}", e),
        }
    });
}

/// Load data from IndexedDB auto-save slot.
/// Calls the callback with Some(bytes) if found, None if no auto-save exists.
#[allow(dead_code)]
pub fn load_auto_save_from_idb(slot: &str, callback: impl FnOnce(Option<Vec<u8>>) + 'static) {
    let slot = slot.to_string();

    wasm_bindgen_futures::spawn_local(async move {
        match idb_get(&slot).await {
            Ok(Some(data)) => {
                log::info!("Auto-save loaded from IndexedDB slot '{}'", slot);
                callback(Some(data));
            }
            Ok(None) => {
                log::info!("No auto-save found in IndexedDB slot '{}'", slot);
                callback(None);
            }
            Err(e) => {
                log::warn!("IndexedDB load failed: {:?}", e);
                callback(None);
            }
        }
    });
}

// ── IndexedDB helpers via raw JS interop ────────────────────────────────

const IDB_NAME: &str = "alkahest-saves";
const IDB_STORE: &str = "auto-save";
const IDB_VERSION: u32 = 1;

async fn open_idb() -> Result<JsValue, JsValue> {
    let window = web_sys::window().expect("no window");
    let idb_factory = js_sys::Reflect::get(&window, &JsValue::from_str("indexedDB"))?;

    let open_fn = js_sys::Reflect::get(&idb_factory, &JsValue::from_str("open"))?;
    let open_fn: js_sys::Function = open_fn.dyn_into()?;
    let request = open_fn.call2(
        &idb_factory,
        &JsValue::from_str(IDB_NAME),
        &JsValue::from_f64(IDB_VERSION as f64),
    )?;

    // Set up onupgradeneeded to create the object store
    let request_clone = request.clone();
    let onupgrade = Closure::<dyn FnMut()>::new(move || {
        let result = js_sys::Reflect::get(&request_clone, &JsValue::from_str("result"))
            .expect("no result on request");
        let create_fn = js_sys::Reflect::get(&result, &JsValue::from_str("createObjectStore"))
            .expect("no createObjectStore");
        let create_fn: js_sys::Function = create_fn.dyn_into().expect("not a function");
        create_fn.call1(&result, &JsValue::from_str(IDB_STORE)).ok();
    });
    js_sys::Reflect::set(
        &request,
        &JsValue::from_str("onupgradeneeded"),
        onupgrade.as_ref(),
    )?;
    onupgrade.forget();

    // Wait for onsuccess
    let (tx, rx) = futures_channel_oneshot();
    let tx = std::cell::RefCell::new(Some(tx));
    let request_for_success = request.clone();
    let onsuccess = Closure::<dyn FnMut()>::new(move || {
        if let Some(sender) = tx.borrow_mut().take() {
            let db = js_sys::Reflect::get(&request_for_success, &JsValue::from_str("result"))
                .expect("no result");
            sender(Ok(db));
        }
    });
    js_sys::Reflect::set(
        &request,
        &JsValue::from_str("onsuccess"),
        onsuccess.as_ref(),
    )?;
    onsuccess.forget();

    rx.await
}

async fn idb_put(key: &str, value: &js_sys::Uint8Array) -> Result<(), JsValue> {
    let db = open_idb().await?;

    let tx_fn = js_sys::Reflect::get(&db, &JsValue::from_str("transaction"))?;
    let tx_fn: js_sys::Function = tx_fn.dyn_into()?;
    let tx = tx_fn.call2(
        &db,
        &JsValue::from_str(IDB_STORE),
        &JsValue::from_str("readwrite"),
    )?;

    let store_fn = js_sys::Reflect::get(&tx, &JsValue::from_str("objectStore"))?;
    let store_fn: js_sys::Function = store_fn.dyn_into()?;
    let store = store_fn.call1(&tx, &JsValue::from_str(IDB_STORE))?;

    let put_fn = js_sys::Reflect::get(&store, &JsValue::from_str("put"))?;
    let put_fn: js_sys::Function = put_fn.dyn_into()?;
    let request = put_fn.call2(&store, value, &JsValue::from_str(key))?;

    let (tx_done, rx_done) = futures_channel_oneshot();
    let tx_done = std::cell::RefCell::new(Some(tx_done));
    let onsuccess = Closure::<dyn FnMut()>::new(move || {
        if let Some(sender) = tx_done.borrow_mut().take() {
            sender(Ok(JsValue::UNDEFINED));
        }
    });
    js_sys::Reflect::set(
        &request,
        &JsValue::from_str("onsuccess"),
        onsuccess.as_ref(),
    )?;
    onsuccess.forget();

    rx_done.await?;
    Ok(())
}

#[allow(dead_code)]
async fn idb_get(key: &str) -> Result<Option<Vec<u8>>, JsValue> {
    let db = open_idb().await?;

    let tx_fn = js_sys::Reflect::get(&db, &JsValue::from_str("transaction"))?;
    let tx_fn: js_sys::Function = tx_fn.dyn_into()?;
    let tx = tx_fn.call2(
        &db,
        &JsValue::from_str(IDB_STORE),
        &JsValue::from_str("readonly"),
    )?;

    let store_fn = js_sys::Reflect::get(&tx, &JsValue::from_str("objectStore"))?;
    let store_fn: js_sys::Function = store_fn.dyn_into()?;
    let store = store_fn.call1(&tx, &JsValue::from_str(IDB_STORE))?;

    let get_fn = js_sys::Reflect::get(&store, &JsValue::from_str("get"))?;
    let get_fn: js_sys::Function = get_fn.dyn_into()?;
    let request = get_fn.call1(&store, &JsValue::from_str(key))?;

    let (tx_done, rx_done) = futures_channel_oneshot();
    let tx_done = std::cell::RefCell::new(Some(tx_done));
    let request_clone = request.clone();
    let onsuccess = Closure::<dyn FnMut()>::new(move || {
        if let Some(sender) = tx_done.borrow_mut().take() {
            let result = js_sys::Reflect::get(&request_clone, &JsValue::from_str("result"))
                .expect("no result");
            sender(Ok(result));
        }
    });
    js_sys::Reflect::set(
        &request,
        &JsValue::from_str("onsuccess"),
        onsuccess.as_ref(),
    )?;
    onsuccess.forget();

    let result = rx_done.await?;
    if result.is_undefined() || result.is_null() {
        return Ok(None);
    }

    let array = js_sys::Uint8Array::new(&result);
    Ok(Some(array.to_vec()))
}

/// Simple oneshot channel using Rc<RefCell<Option>> and a waker closure.
/// Returns (sender, future).
fn futures_channel_oneshot() -> (
    impl FnOnce(Result<JsValue, JsValue>),
    impl std::future::Future<Output = Result<JsValue, JsValue>>,
) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::task::{Poll, Waker};

    struct State {
        value: Option<Result<JsValue, JsValue>>,
        waker: Option<Waker>,
    }

    let state = Rc::new(RefCell::new(State {
        value: None,
        waker: None,
    }));

    let sender_state = state.clone();
    let sender = move |val: Result<JsValue, JsValue>| {
        let mut s = sender_state.borrow_mut();
        s.value = Some(val);
        if let Some(waker) = s.waker.take() {
            waker.wake();
        }
    };

    let receiver = {
        struct Receiver {
            state: Rc<RefCell<State>>,
        }

        impl std::future::Future for Receiver {
            type Output = Result<JsValue, JsValue>;

            fn poll(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> Poll<Self::Output> {
                let mut s = self.state.borrow_mut();
                if let Some(val) = s.value.take() {
                    Poll::Ready(val)
                } else {
                    s.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }

        Receiver { state }
    };

    (sender, receiver)
}

//! JNI Bridge for WordCraft Android.
//!
//! Exposes:
//! 1. High-frequency hot path: raw primitives for touch, pinch-zoom, fling, handle drag, and IME.
//! 2. Uniform command dispatch: 389+ discrete WordCraft engine commands.
//! 3. Bi-directional state sync: extracts `EditorUiState` for Kotlin StateFlow.
//! 4. Surface lifecycle: attaches and renders onto Android `SurfaceView` via `ANativeWindow`.
//! 5. Scoped Storage (SAF): loads and exports documents via byte buffers.

use std::sync::atomic::{AtomicBool, Ordering};
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jboolean, jbyteArray, jfloat, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use log::{error, info};

use wordcraft_doc::StoryRef;
use wordcraft_engine::Session;
use crate::editor_state::{extract_ui_state, hit_test_screen};
use crate::ime_bridge::{
    commit_text, delete_surrounding_text, finish_composing_text, selected_text,
    set_composing_text, text_after_cursor, text_before_cursor, AndroidInsets,
};
use crate::render_surface::AndroidRenderer;
use crate::storage_bridge::{export_document_pdf, open_document_bytes, save_document_bytes};
use crate::touch::{GestureEvent, SelectionHandle, TouchRecognizer};

static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);

fn init_logging() {
    if !LOGGER_INITIALIZED.swap(true, Ordering::SeqCst) {
        #[cfg(target_os = "android")]
        {
            let _ = android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Info)
                    .with_tag("WordCraftRust"),
            );
        }
        #[cfg(not(target_os = "android"))]
        {
            // Host logging no-op
        }
    }
}

/// Host struct containing the WordCraft session, renderer, touch recognizer, and viewport state.
pub struct WordcraftHost {
    pub session: Session,
    pub renderer: Option<AndroidRenderer>,
    pub touch_recognizer: TouchRecognizer,
    pub insets: AndroidInsets,
    pub zoom: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub composing_chars: usize,
    #[cfg(target_os = "android")]
    pub native_window: Option<*mut ndk_sys::ANativeWindow>,
    #[cfg(not(target_os = "android"))]
    pub native_window: Option<*mut std::ffi::c_void>,
}

impl WordcraftHost {
    pub fn new(sample: bool) -> Self {
        init_logging();
        let region = wordcraft_geom::Region::default();
        let doc = if sample {
            wordcraft_engine::sample::sample_document()
        } else {
            wordcraft_engine::blank_document(&region)
        };
        let session = Session::new(doc);
        Self {
            session,
            renderer: None,
            touch_recognizer: TouchRecognizer::new(),
            insets: AndroidInsets::default(),
            zoom: 1.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            composing_chars: 0,
            native_window: None,
        }
    }

    pub fn clamp_viewport(&mut self) {
        self.zoom = self.zoom.clamp(0.5, 4.0);
        let layout = self.session.layout();
        let page_count = layout.pages.len().max(1);
        let max_page_w = layout.pages.iter().map(|p| p.w).fold(612.0_f32, f32::max);
        let total_doc_h = layout.pages.iter().map(|p| p.h).sum::<f32>() + (page_count as f32 * 18.0);

        let max_scroll_x = (max_page_w * self.zoom - 100.0).max(0.0);
        let max_scroll_y = (total_doc_h * self.zoom - 200.0).max(0.0);

        self.scroll_x = self.scroll_x.clamp(-100.0, max_scroll_x + 100.0);
        self.scroll_y = self.scroll_y.clamp(-100.0, max_scroll_y + 100.0);
    }
}

// Helper to safely extract host reference from raw jlong
fn host_from_ptr<'a>(ptr: jlong) -> Option<&'a mut WordcraftHost> {
    if ptr == 0 {
        None
    } else {
        unsafe { Some(&mut *(ptr as *mut WordcraftHost)) }
    }
}

// -----------------------------------------------------------------------------
// Lifecycle & Storage
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeInit(
    _env: JNIEnv,
    _class: JClass,
    sample: jboolean,
) -> jlong {
    let host = Box::new(WordcraftHost::new(sample == JNI_TRUE));
    Box::into_raw(host) as jlong
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    if ptr != 0 {
        let mut host = unsafe { Box::from_raw(ptr as *mut WordcraftHost) };
        if let Some(renderer) = host.renderer.as_mut() {
            renderer.detach_surface();
        }
        #[cfg(target_os = "android")]
        if let Some(win) = host.native_window.take() {
            unsafe { ndk_sys::ANativeWindow_release(win) };
        }
        info!("WordcraftHost cleanly dropped");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOpenDocument(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    filename: JString,
    bytes: JByteArray,
    password: JString,
) -> jboolean {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return JNI_FALSE,
    };

    let filename_str: String = match env.get_string(&filename) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };

    let password_opt: Option<String> = if password.is_null() {
        None
    } else {
        env.get_string(&password).ok().map(|s| s.into())
    };

    let byte_vec = match env.convert_byte_array(&bytes) {
        Ok(v) => v,
        Err(_) => return JNI_FALSE,
    };

    let region = wordcraft_geom::Region::default();
    match open_document_bytes(&filename_str, &byte_vec, password_opt.as_deref(), &region) {
        Ok(doc) => {
            host.session = Session::new(doc);
            host.scroll_x = 0.0;
            host.scroll_y = 0.0;
            host.zoom = 1.0;
            host.composing_chars = 0;
            info!("Successfully opened document: {filename_str}");
            JNI_TRUE
        }
        Err(e) => {
            error!("Failed to open document {filename_str}: {e}");
            JNI_FALSE
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeSaveDocument(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    filename: JString,
    password: JString,
) -> jbyteArray {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };

    let filename_str: String = match env.get_string(&filename) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let password_opt: Option<String> = if password.is_null() {
        None
    } else {
        env.get_string(&password).ok().map(|s| s.into())
    };

    match save_document_bytes(&filename_str, &host.session.doc, password_opt.as_deref(), None) {
        Ok(bytes) => match env.byte_array_from_slice(&bytes) {
            Ok(arr) => arr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(e) => {
            error!("Failed to save document {filename_str}: {e}");
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeExportPdf(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    tagged: jboolean,
    markup: jboolean,
    comments: jboolean,
) -> jbyteArray {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };

    match export_document_pdf(
        &host.session.doc,
        tagged == JNI_TRUE,
        markup == JNI_TRUE,
        comments == JNI_TRUE,
    ) {
        Ok(bytes) => match env.byte_array_from_slice(&bytes) {
            Ok(arr) => arr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(e) => {
            error!("Failed to export PDF: {e}");
            std::ptr::null_mut()
        }
    }
}

// -----------------------------------------------------------------------------
// Unified Command Dispatch
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeDispatchCommand(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    cmd_id: JString,
    params_json: JString,
) -> jstring {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };

    let cmd_str: String = match env.get_string(&cmd_id) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    let params_val: serde_json::Value = if params_json.is_null() {
        serde_json::Value::Null
    } else {
        match env.get_string(&params_json) {
            Ok(s) => {
                let s_str: String = s.into();
                if s_str.trim().is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_str(&s_str).unwrap_or(serde_json::Value::Null)
                }
            }
            Err(_) => serde_json::Value::Null,
        }
    };

    let result = host.session.run(&cmd_str, &params_val);
    let output_json = match result {
        Ok(val) => serde_json::json!({ "status": "ok", "result": val }).to_string(),
        Err(e) => serde_json::json!({ "status": "error", "error": format!("{e:?}") }).to_string(),
    };

    match env.new_string(output_json) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// -----------------------------------------------------------------------------
// Surface & Rendering
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeAttachSurface(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    surface: JObject,
    width: jint,
    height: jint,
) -> jboolean {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return JNI_FALSE,
    };

    info!("nativeAttachSurface: {width}x{height}");

    #[cfg(target_os = "android")]
    {
        let raw_env = _env.get_raw();
        let raw_surface = surface.as_raw();
        let native_window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(raw_env as *mut _, raw_surface as *mut _)
        };
        if native_window.is_null() {
            error!("ANativeWindow_fromSurface returned null");
            return JNI_FALSE;
        }

        // Release prior window if any
        if let Some(old_win) = host.native_window.take() {
            unsafe { ndk_sys::ANativeWindow_release(old_win) };
        }
        host.native_window = Some(native_window);

        let window_handle = match std::ptr::NonNull::new(native_window as *mut std::ffi::c_void) {
            Some(nn) => raw_window_handle::AndroidNdkWindowHandle::new(nn),
            None => return JNI_FALSE,
        };

        let raw_handle = raw_window_handle::RawWindowHandle::AndroidNdk(window_handle);
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: None,
            raw_window_handle: raw_handle,
        };

        // Initialize renderer if not yet present
        if host.renderer.is_none() {
            match pollster::block_on(AndroidRenderer::new()) {
                Ok(r) => host.renderer = Some(r),
                Err(e) => {
                    error!("Failed to create AndroidRenderer: {e}");
                    return JNI_FALSE;
                }
            }
        }

        if let Some(renderer) = host.renderer.as_mut() {
            if let Err(e) = renderer.attach_surface_unsafe(target, width.max(1) as u32, height.max(1) as u32) {
                error!("Failed to attach surface unsafe: {e}");
                return JNI_FALSE;
            }
        }
        JNI_TRUE
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (_env, surface, width, height, host);
        info!("Host dummy nativeAttachSurface ok");
        JNI_TRUE
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeDetachSurface(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };

    if let Some(renderer) = host.renderer.as_mut() {
        renderer.detach_surface();
    }
    #[cfg(target_os = "android")]
    if let Some(win) = host.native_window.take() {
        unsafe { ndk_sys::ANativeWindow_release(win) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeResizeSurface(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    width: jint,
    height: jint,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };

    if let Some(renderer) = host.renderer.as_mut() {
        renderer.resize_surface(width.max(1) as u32, height.max(1) as u32);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeRenderFrame(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };

    let zoom = host.zoom;
    let sx = host.scroll_x;
    let sy = host.scroll_y;
    let session = &mut host.session;

    if let Some(renderer) = host.renderer.as_mut() {
        let _ = renderer.render_frame(session, zoom, sx, sy);
    }
}

// -----------------------------------------------------------------------------
// Touch Gestures & Text Selection
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOnTouch(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    action: jint,
    pointer_id: jint,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return JNI_FALSE,
    };

    let pointers = [(pointer_id, x, y)];
    let ui_state = extract_ui_state(&mut host.session, host.zoom, host.scroll_x, host.scroll_y);

    let start_handle = if ui_state.has_selection {
        Some(SelectionHandle {
            x: ui_state.selection_start_x,
            y: ui_state.selection_start_y,
            width: 32.0,
            height: 32.0,
            is_start: true,
        })
    } else {
        None
    };

    let end_handle = if ui_state.has_selection {
        Some(SelectionHandle {
            x: ui_state.selection_end_x,
            y: ui_state.selection_end_y,
            width: 32.0,
            height: 32.0,
            is_start: false,
        })
    } else {
        None
    };

    let gesture = host.touch_recognizer.on_touch(action, &pointers, start_handle, end_handle);

    match gesture {
        GestureEvent::Tap { x, y } => {
            let layout = host.session.layout();
            if let Some(pos) = hit_test_screen(&layout, x, y, host.zoom, host.scroll_x, host.scroll_y, StoryRef::Body) {
                host.session.sel = wordcraft_engine::Selection::caret(pos);
            }
            JNI_TRUE
        }
        GestureEvent::DoubleTap { x, y } | GestureEvent::LongPress { x, y } => {
            let layout = host.session.layout();
            if let Some(pos) = hit_test_screen(&layout, x, y, host.zoom, host.scroll_x, host.scroll_y, StoryRef::Body) {
                host.session.sel = wordcraft_engine::Selection::caret(pos);
                let _ = host.session.run("select.word", &serde_json::json!({}));
            }
            JNI_TRUE
        }
        GestureEvent::Scroll { dx, dy } => {
            host.scroll_x -= dx;
            host.scroll_y -= dy;
            host.clamp_viewport();
            JNI_TRUE
        }
        GestureEvent::Zoom { scale_factor, focus_x, focus_y } => {
            let old_zoom = host.zoom;
            let new_zoom = (old_zoom * scale_factor).clamp(0.5, 4.0);
            let ratio = new_zoom / old_zoom;
            host.scroll_x = (focus_x + host.scroll_x) * ratio - focus_x;
            host.scroll_y = (focus_y + host.scroll_y) * ratio - focus_y;
            host.zoom = new_zoom;
            host.clamp_viewport();
            JNI_TRUE
        }
        GestureEvent::DragHandle { is_start, x, y } => {
            let role = if is_start { 0 } else { 1 };
            let _ = drag_handle_internal(host, role, x, y);
            JNI_TRUE
        }
        GestureEvent::None => JNI_FALSE,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOnScroll(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    dx: jfloat,
    dy: jfloat,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    host.scroll_x -= dx;
    host.scroll_y -= dy;
    host.clamp_viewport();
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOnScale(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    factor: jfloat,
    focus_x: jfloat,
    focus_y: jfloat,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    let old_zoom = host.zoom;
    let new_zoom = (old_zoom * factor).clamp(0.5, 4.0);
    let ratio = new_zoom / old_zoom;
    host.scroll_x = (focus_x + host.scroll_x) * ratio - focus_x;
    host.scroll_y = (focus_y + host.scroll_y) * ratio - focus_y;
    host.zoom = new_zoom;
    host.clamp_viewport();
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOnFling(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    vx: jfloat,
    vy: jfloat,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    host.touch_recognizer.fling.vx = vx;
    host.touch_recognizer.fling.vy = vy;
    host.touch_recognizer.fling.is_active = true;
}

fn drag_handle_internal(host: &mut WordcraftHost, handle_role: i32, x: f32, y: f32) -> i32 {
    // Finger occlusion offset: the finger bulb is below the text line
    let adjusted_y = (y - 36.0).max(0.0);
    let layout = host.session.layout();
    let pos = match hit_test_screen(&layout, x, adjusted_y, host.zoom, host.scroll_x, host.scroll_y, StoryRef::Body) {
        Some(p) => p,
        None => return 0,
    };

    let anchor = host.session.sel.anchor.clone();
    let focus = host.session.sel.focus.clone();

    if handle_role == 0 {
        // Dragging start handle (anchor)
        if pos > focus {
            // Crossover! Anchor flipped past focus
            host.session.sel.anchor = focus;
            host.session.sel.focus = pos;
            2 // Flipped
        } else {
            host.session.sel.anchor = pos;
            1 // Updated
        }
    } else {
        // Dragging end handle (focus)
        if pos < anchor {
            // Crossover! Focus flipped before anchor
            host.session.sel.focus = anchor;
            host.session.sel.anchor = pos;
            2 // Flipped
        } else {
            host.session.sel.focus = pos;
            1 // Updated
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeOnDragSelectionHandle(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    handle_role: jint,
    x: jfloat,
    y: jfloat,
) -> jint {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return 0,
    };
    drag_handle_internal(host, handle_role, x, y)
}

// -----------------------------------------------------------------------------
// IME & Soft Keyboard
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeCommitText(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    text: JString,
    _new_cursor_pos: jint,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };

    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    let _ = commit_text(&mut host.session, &text_str, &mut host.composing_chars);
    ensure_caret_visible_internal(host);
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeSetComposingText(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    text: JString,
    _new_cursor_pos: jint,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };

    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    let _ = set_composing_text(&mut host.session, &text_str, &mut host.composing_chars);
    ensure_caret_visible_internal(host);
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeFinishComposingText(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    finish_composing_text(&mut host.composing_chars);
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeDeleteSurroundingText(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    before: jint,
    after: jint,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    let _ = delete_surrounding_text(&mut host.session, before.max(0) as usize, after.max(0) as usize);
    ensure_caret_visible_internal(host);
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeBeginBatchEdit(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    host.session.join_next_undo();
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeEndBatchEdit(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    host.session.continue_typing();
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeGetTextBeforeCursor(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    n: jint,
) -> jstring {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };
    let s = text_before_cursor(&host.session, n.max(0) as usize);
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeGetTextAfterCursor(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    n: jint,
) -> jstring {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };
    let s = text_after_cursor(&host.session, n.max(0) as usize);
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeImeGetSelectedText(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) -> jstring {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };
    let s = selected_text(&host.session);
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// -----------------------------------------------------------------------------
// WindowInsets & Caret Auto-Scroll
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeUpdateInsets(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    status_bar: jfloat,
    nav_bar: jfloat,
    ime_height: jfloat,
    screen_width: jfloat,
    screen_height: jfloat,
) {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return,
    };
    host.insets = AndroidInsets {
        status_bar,
        nav_bar,
        ime_height,
        screen_width,
        screen_height,
    };
    ensure_caret_visible_internal(host);
}

fn ensure_caret_visible_internal(host: &mut WordcraftHost) -> bool {
    let zoom = host.zoom;
    let sx = host.scroll_x;
    let sy = host.scroll_y;
    let insets = host.insets;
    let ui_state = extract_ui_state(&mut host.session, zoom, sx, sy);
    if let Some(target_sy) = insets.ensure_caret_visible(ui_state.caret_screen_y, ui_state.caret_height, sy) {
        host.scroll_y = target_sy;
        host.clamp_viewport();
        true
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeEnsureCaretVisible(
    _env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) -> jboolean {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return JNI_FALSE,
    };
    if ensure_caret_visible_internal(host) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// -----------------------------------------------------------------------------
// Reactive State Sync
// -----------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn Java_ai_storyteller_wordcraft_WordcraftBridge_nativeGetUiStateJson(
    env: JNIEnv,
    _class: JClass,
    ptr: jlong,
) -> jstring {
    let host = match host_from_ptr(ptr) {
        Some(h) => h,
        None => return std::ptr::null_mut(),
    };

    let zoom = host.zoom;
    let sx = host.scroll_x;
    let sy = host.scroll_y;
    let state = extract_ui_state(&mut host.session, zoom, sx, sy);
    let json_str = match serde_json::to_string(&state) {
        Ok(s) => s,
        Err(_) => "{}".to_string(),
    };

    match env.new_string(json_str) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

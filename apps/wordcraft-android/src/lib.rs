//! WordCraft Android native crate: hybrid Rust core + WGPU canvas renderer + JNI bridge.

pub mod editor_state;
pub mod ime_bridge;
pub mod render_surface;
pub mod storage_bridge;
pub mod touch;
pub mod jni_bridge;

pub use editor_state::{EditorUiState, extract_ui_state, hit_test_screen, compute_page_top};
pub use ime_bridge::{
    AndroidInsets, commit_text, delete_surrounding_text, finish_composing_text,
    selected_text, set_composing_text, text_after_cursor, text_before_cursor,
};
pub use render_surface::AndroidRenderer;
pub use storage_bridge::{export_document_pdf, open_document_bytes, save_document_bytes};
pub use touch::{FloatingToolbar, GestureEvent, SelectionHandle, TouchRecognizer};

//! Android Soft Keyboard (IME) and WindowInsets Bridge.
//!
//! Handles:
//! - WindowInsets integration (keyboard height, status/nav bar insets).
//! - Viewport auto-scroll so the caret is never occluded by the soft keyboard.
//! - IME composition and commit events for swipe typing, predictive text, and multi-language input.
//! - Text context extraction around caret (`getTextBeforeCursor`, `getTextAfterCursor`, `getSelectedText`)
//!   for Gboard autocompletion and next-word prediction.
//! - Transactional batch editing grouping continuous swipe input into single clean undo steps.

use log::info;
use serde_json::json;
use wordcraft_engine::Session;

/// Android Window Insets in physical/logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AndroidInsets {
    pub status_bar: f32,
    pub nav_bar: f32,
    pub ime_height: f32,
    pub screen_width: f32,
    pub screen_height: f32,
}

impl AndroidInsets {
    /// Computes the usable vertical area for the document canvas above the keyboard.
    pub fn visible_height(&self) -> f32 {
        (self.screen_height - self.status_bar - self.nav_bar - self.ime_height).max(100.0)
    }

    /// Calculates required scroll adjustment to bring the caret into view.
    /// `caret_screen_y` is the caret's current vertical coordinate in screen space.
    pub fn ensure_caret_visible(&self, caret_screen_y: f32, caret_height: f32, current_scroll_y: f32) -> Option<f32> {
        if self.screen_height <= 0.0 {
            return None;
        }
        let visible_top = self.status_bar;
        let visible_bottom = self.screen_height - self.nav_bar - self.ime_height;
        let margin = 32.0; // Comfort margin in points/pixels

        if caret_screen_y - margin < visible_top {
            // Caret is hidden above the top visible boundary: scroll down
            let diff = visible_top - (caret_screen_y - margin);
            Some((current_scroll_y - diff).max(0.0))
        } else if caret_screen_y + caret_height + margin > visible_bottom {
            // Caret is occluded below by on-screen keyboard: scroll up
            let diff = (caret_screen_y + caret_height + margin) - visible_bottom;
            Some(current_scroll_y + diff)
        } else {
            None
        }
    }
}

/// Dispatches an IME text commit event into WordCraft's engine.
/// Cleans up any prior in-flight composition and commits the text.
pub fn commit_text(session: &mut Session, text: &str, composing_chars: &mut usize) -> Result<(), String> {
    info!("IME commit text: '{text}' (replacing {composing_chars} composing chars)");
    if *composing_chars > 0 {
        delete_surrounding_text(session, *composing_chars, 0)?;
        *composing_chars = 0;
    }
    session
        .run("text.insert", &json!({ "text": text }))
        .map_err(|e| format!("Failed to commit IME text: {e:?}"))?;
    Ok(())
}

/// Updates in-flight composing text (preview before commit during swipe-typing or Pinyin).
/// Joins previous undo steps so intermediate keystrokes do not pollute the undo stack.
pub fn set_composing_text(session: &mut Session, text: &str, composing_chars: &mut usize) -> Result<(), String> {
    info!("IME set composing text: '{text}' (current composing={composing_chars})");
    if *composing_chars > 0 {
        session.join_next_undo();
        delete_surrounding_text(session, *composing_chars, 0)?;
    }
    if !text.is_empty() {
        session.join_next_undo();
        session
            .run("text.insert", &json!({ "text": text }))
            .map_err(|e| format!("Failed to set composing text: {e:?}"))?;
    }
    *composing_chars = text.chars().count();
    Ok(())
}

/// Finalizes the current composition without changing text.
pub fn finish_composing_text(composing_chars: &mut usize) {
    info!("IME finish composing text (cleared {composing_chars} tracking)");
    *composing_chars = 0;
}

/// Dispatches an IME backspace / delete surrounding text event.
pub fn delete_surrounding_text(session: &mut Session, before: usize, after: usize) -> Result<(), String> {
    info!("IME delete surrounding text before={before}, after={after}");
    for _ in 0..before {
        session
            .run("text.backspace", &json!({}))
            .map_err(|e| format!("Failed to backspace: {e:?}"))?;
    }
    for _ in 0..after {
        session
            .run("text.delete", &json!({}))
            .map_err(|e| format!("Failed to forward delete: {e:?}"))?;
    }
    Ok(())
}

/// Extracts up to `n` characters immediately preceding the caret for IME autocompletion and prediction.
pub fn text_before_cursor(session: &Session, n: usize) -> String {
    let focus = &session.sel.focus;
    if let Some(para) = session.doc.para_at(focus) {
        let off = focus.off.min(para.text.len());
        let before_bytes = &para.text[..off];
        before_bytes.chars().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()
    } else {
        String::new()
    }
}

/// Extracts up to `n` characters immediately following the caret for IME context.
pub fn text_after_cursor(session: &Session, n: usize) -> String {
    let focus = &session.sel.focus;
    if let Some(para) = session.doc.para_at(focus) {
        let off = focus.off.min(para.text.len());
        let after_bytes = &para.text[off..];
        after_bytes.chars().take(n).collect()
    } else {
        String::new()
    }
}

/// Returns the plain text of the active selection (for clipboard and IME replacement).
pub fn selected_text(session: &Session) -> String {
    session.selected_text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_insets_visible_height() {
        let insets = AndroidInsets {
            status_bar: 48.0,
            nav_bar: 48.0,
            ime_height: 300.0,
            screen_width: 1080.0,
            screen_height: 1920.0,
        };
        // 1920 - 48 - 48 - 300 = 1524
        assert_eq!(insets.visible_height(), 1524.0);
    }

    #[test]
    fn test_ensure_caret_visible_auto_scroll() {
        let insets = AndroidInsets {
            status_bar: 50.0,
            nav_bar: 50.0,
            ime_height: 500.0,
            screen_width: 1080.0,
            screen_height: 1920.0,
        };
        // Visible area: top = 50.0, bottom = 1920 - 50 - 500 = 1370.0
        let current_scroll = 100.0;

        // Caret comfortably visible at y = 600.0
        assert_eq!(insets.ensure_caret_visible(600.0, 20.0, current_scroll), None);

        // Caret occluded below keyboard at y = 1400.0 (past bottom 1370.0)
        // Required scroll up: diff = (1400 + 20 + 32) - 1370 = 82.0 -> target_scroll = 100 + 82 = 182.0
        let scroll_up = insets.ensure_caret_visible(1400.0, 20.0, current_scroll);
        assert!(scroll_up.is_some());
        assert_eq!(scroll_up.unwrap(), 182.0);

        // Caret hidden above status bar at y = 20.0
        // Required scroll down: diff = 50 - (20 - 32) = 62.0 -> target_scroll = 100 - 62 = 38.0
        let scroll_down = insets.ensure_caret_visible(20.0, 20.0, current_scroll);
        assert!(scroll_down.is_some());
        assert_eq!(scroll_down.unwrap(), 38.0);
    }

    #[test]
    fn test_ime_text_before_and_after_cursor() {
        let region = wordcraft_geom::Region::default();
        let doc = wordcraft_engine::blank_document(&region);
        let mut session = Session::new(doc);

        let _ = session.run("text.insert", &json!({ "text": "Hello World" }));

        // Caret is at end of "Hello World"
        assert_eq!(text_before_cursor(&session, 5), "World");
        assert_eq!(text_before_cursor(&session, 11), "Hello World");
        assert_eq!(text_after_cursor(&session, 5), "");

        // Move caret back 5 characters (between "Hello " and "World")
        for _ in 0..5 {
            let _ = session.run("caret.left", &json!({}));
        }
        assert_eq!(text_before_cursor(&session, 6), "Hello ");
        assert_eq!(text_after_cursor(&session, 5), "World");
    }

    #[test]
    fn test_composing_text_lifecycle() {
        let region = wordcraft_geom::Region::default();
        let doc = wordcraft_engine::blank_document(&region);
        let mut session = Session::new(doc);
        let mut composing_chars = 0;

        // Start composing "wor"
        set_composing_text(&mut session, "wor", &mut composing_chars).unwrap();
        assert_eq!(composing_chars, 3);
        assert_eq!(text_before_cursor(&session, 3), "wor");

        // Update composing text to "world"
        set_composing_text(&mut session, "world", &mut composing_chars).unwrap();
        assert_eq!(composing_chars, 5);
        assert_eq!(text_before_cursor(&session, 5), "world");

        // Commit "world "
        commit_text(&mut session, "world ", &mut composing_chars).unwrap();
        assert_eq!(composing_chars, 0);
        assert_eq!(text_before_cursor(&session, 6), "world ");
    }
}

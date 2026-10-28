//! Extraction of lightweight EditorUiState from WordCraft Session.
//!
//! Provides snapshotting of document state, formatting flags, and screen-space
//! selection / caret coordinates for reactive Jetpack Compose UI updates.

use serde::{Deserialize, Serialize};
use wordcraft_doc::props::Align;
use wordcraft_engine::Session;

/// Lightweight UI state synchronized reactively across JNI into Kotlin StateFlow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorUiState {
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_underline: bool,
    pub is_strikethrough: bool,
    pub alignment: i32, // 0 = Left, 1 = Center, 2 = Right, 3 = Justify
    pub style_name: String,
    pub font_name: String,
    pub font_size_pt: f32,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_dirty: bool,
    pub page_count: i32,
    pub word_count: i32,
    pub has_selection: bool,

    // Screen-space coordinates for Compose overlays (handles & floating toolbar)
    pub selection_start_x: f32,
    pub selection_start_y: f32,
    pub selection_end_x: f32,
    pub selection_end_y: f32,
    pub selection_bounds_min_x: f32,
    pub selection_bounds_min_y: f32,
    pub selection_bounds_max_x: f32,
    pub selection_bounds_max_y: f32,

    pub caret_screen_x: f32,
    pub caret_screen_y: f32,
    pub caret_height: f32,
}

impl Default for EditorUiState {
    fn default() -> Self {
        Self {
            is_bold: false,
            is_italic: false,
            is_underline: false,
            is_strikethrough: false,
            alignment: 0,
            style_name: "Normal".to_string(),
            font_name: "Calibri".to_string(),
            font_size_pt: 11.0,
            can_undo: false,
            can_redo: false,
            is_dirty: false,
            page_count: 1,
            word_count: 0,
            has_selection: false,
            selection_start_x: 0.0,
            selection_start_y: 0.0,
            selection_end_x: 0.0,
            selection_end_y: 0.0,
            selection_bounds_min_x: 0.0,
            selection_bounds_min_y: 0.0,
            selection_bounds_max_x: 0.0,
            selection_bounds_max_y: 0.0,
            caret_screen_x: 0.0,
            caret_screen_y: 0.0,
            caret_height: 14.0,
        }
    }
}

/// Calculates the top screen-space coordinate of a page including zoom, inter-page gaps, and scroll offset.
pub fn compute_page_top(pages: &[wordcraft_layout::Page], page_idx: usize, zoom: f32, scroll_y: f32) -> f32 {
    let page_gap = 18.0 * zoom;
    let mut top = 0.0;
    for i in 0..page_idx {
        if let Some(p) = pages.get(i) {
            top += p.h * zoom + page_gap;
        }
    }
    top - scroll_y
}

/// Hit-tests a screen point across all laid-out pages in document space.
pub fn hit_test_screen(
    layout: &wordcraft_layout::DocLayout,
    x: f32,
    y: f32,
    zoom: f32,
    scroll_x: f32,
    scroll_y: f32,
    story: wordcraft_doc::StoryRef,
) -> Option<wordcraft_doc::Pos> {
    if layout.pages.is_empty() {
        return None;
    }
    let total_doc_y = y + scroll_y;
    let page_gap = 18.0 * zoom;

    let mut current_y = 0.0;
    let mut target_page = 0;
    let mut page_local_y = 0.0;

    for (idx, page) in layout.pages.iter().enumerate() {
        let page_h_scaled = page.h * zoom;
        let next_y = current_y + page_h_scaled + page_gap;
        if total_doc_y <= next_y || idx == layout.pages.len() - 1 {
            target_page = idx;
            page_local_y = ((total_doc_y - current_y) / zoom).clamp(0.0, page.h);
            break;
        }
        current_y = next_y;
    }

    let target_page_layout = layout.pages.get(target_page)?;
    let page_local_x = ((x + scroll_x) / zoom).clamp(0.0, target_page_layout.w);

    layout.hit(target_page, page_local_x, page_local_y, story)
}

/// Computes the current EditorUiState from the Session, viewport zoom, and scroll offsets.
pub fn extract_ui_state(session: &mut Session, zoom: f32, scroll_x: f32, scroll_y: f32) -> EditorUiState {
    let typing = session.typing_props();
    let current_para = session.doc.para_at(&session.sel.focus);

    let is_bold = typing.bold.unwrap_or(false);
    let is_italic = typing.italic.unwrap_or(false);
    let is_underline = typing.underline.is_some();
    let is_strikethrough = typing.strike.unwrap_or(false);

    let alignment = match current_para.and_then(|p| p.props.align) {
        Some(Align::Center) => 1,
        Some(Align::Right) => 2,
        Some(Align::Justify) | Some(Align::Distribute) => 3,
        _ => 0, // Left / default
    };

    let style_name = current_para
        .and_then(|p| p.props.style.clone())
        .unwrap_or_else(|| "Normal".to_string());

    let font_name = typing
        .font
        .clone()
        .or_else(|| typing.font_cs.clone())
        .unwrap_or_else(|| "Calibri".to_string());

    let font_size_pt = typing.size.unwrap_or(11.0);

    let can_undo = session.can_undo();
    let can_redo = session.can_redo();
    let is_dirty = session.dirty;
    let word_count = session.doc.word_count() as i32;

    let page_hint = session.page_hint;
    let focus = session.sel.focus.clone();
    let (a, b) = session.sel.ordered();
    let has_selection = a != b;

    let layout = session.layout();
    let page_count = layout.pages.len() as i32;

    let mut caret_screen_x = 0.0;
    let mut caret_screen_y = 0.0;
    let mut caret_height = 14.0 * zoom;

    // Calculate caret screen position
    if let Some(c) = layout.caret_on(&focus, page_hint) {
        let page_top = compute_page_top(&layout.pages, c.page, zoom, scroll_y);
        caret_screen_x = (c.x * zoom) - scroll_x;
        caret_screen_y = page_top + (c.top * zoom);
        caret_height = c.height.max(12.0) * zoom;
    }

    let mut sel_start_x = 0.0;
    let mut sel_start_y = 0.0;
    let mut sel_end_x = 0.0;
    let mut sel_end_y = 0.0;
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    if has_selection {
        let rects = layout.selection_rects(&session.doc, &a, &b, page_hint);
        for (page_idx, r) in &rects {
            let page_top = compute_page_top(&layout.pages, *page_idx, zoom, scroll_y);
            let sx = (r.x * zoom) - scroll_x;
            let sy = page_top + (r.y * zoom);
            let sw = r.w * zoom;
            let sh = r.h * zoom;

            min_x = min_x.min(sx);
            min_y = min_y.min(sy);
            max_x = max_x.max(sx + sw);
            max_y = max_y.max(sy + sh);
        }

        // Exact caret positions for selection start and end handles
        if let Some(ca) = layout.caret_on(&a, page_hint) {
            let page_top = compute_page_top(&layout.pages, ca.page, zoom, scroll_y);
            sel_start_x = (ca.x * zoom) - scroll_x;
            sel_start_y = page_top + ((ca.top + ca.height) * zoom);
        } else if let Some((first_page, first_r)) = rects.first() {
            let page_top = compute_page_top(&layout.pages, *first_page, zoom, scroll_y);
            sel_start_x = (first_r.x * zoom) - scroll_x;
            sel_start_y = page_top + ((first_r.y + first_r.h) * zoom);
        }

        if let Some(cb) = layout.caret_on(&b, page_hint) {
            let page_top = compute_page_top(&layout.pages, cb.page, zoom, scroll_y);
            sel_end_x = (cb.x * zoom) - scroll_x;
            sel_end_y = page_top + ((cb.top + cb.height) * zoom);
        } else if let Some((last_page, last_r)) = rects.last() {
            let page_top = compute_page_top(&layout.pages, *last_page, zoom, scroll_y);
            sel_end_x = ((last_r.x + last_r.w) * zoom) - scroll_x;
            sel_end_y = page_top + ((last_r.y + last_r.h) * zoom);
        }
    }

    if min_x == f32::MAX {
        min_x = caret_screen_x;
        min_y = caret_screen_y;
        max_x = caret_screen_x;
        max_y = caret_screen_y + caret_height;
    }

    EditorUiState {
        is_bold,
        is_italic,
        is_underline,
        is_strikethrough,
        alignment,
        style_name,
        font_name,
        font_size_pt,
        can_undo,
        can_redo,
        is_dirty,
        page_count,
        word_count,
        has_selection,
        selection_start_x: sel_start_x,
        selection_start_y: sel_start_y,
        selection_end_x: sel_end_x,
        selection_end_y: sel_end_y,
        selection_bounds_min_x: min_x,
        selection_bounds_min_y: min_y,
        selection_bounds_max_x: max_x,
        selection_bounds_max_y: max_y,
        caret_screen_x,
        caret_screen_y,
        caret_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_page_top() {
        assert_eq!(compute_page_top(&[], 0, 1.0, 0.0), 0.0);
        assert_eq!(compute_page_top(&[], 0, 1.0, 50.0), -50.0);
    }

    #[test]
    fn test_extract_ui_state_blank_document() {
        let region = wordcraft_geom::Region::default();
        let doc = wordcraft_engine::blank_document(&region);
        let mut session = Session::new(doc);

        let state = extract_ui_state(&mut session, 1.0, 0.0, 0.0);
        assert_eq!(state.style_name, "Normal");
        assert_eq!(state.page_count, 1);
        assert!(!state.has_selection);
        assert!(!state.can_undo);
        assert!(!state.can_redo);
    }
}

package ai.storyteller.wordcraft

import org.json.JSONObject

/**
 * Lightweight, reactive UI state synchronized across JNI into Kotlin StateFlow.
 * Drives Compose Material 3 Chrome (Top Bar, Bottom Formatting Bar, Teardrop Handles).
 */
data class EditorUiState(
    val isBold: Boolean = false,
    val isItalic: Boolean = false,
    val isUnderline: Boolean = false,
    val isStrikethrough: Boolean = false,
    val alignment: Int = 0, // 0 = Left, 1 = Center, 2 = Right, 3 = Justify
    val styleName: String = "Normal",
    val fontName: String = "Calibri",
    val fontSizePt: Float = 11.0f,
    val canUndo: Boolean = false,
    val canRedo: Boolean = false,
    val isDirty: Boolean = false,
    val pageCount: Int = 1,
    val wordCount: Int = 0,
    val hasSelection: Boolean = false,

    // Screen-space coordinates for Compose overlays
    val selectionStartX: Float = 0f,
    val selectionStartY: Float = 0f,
    val selectionEndX: Float = 0f,
    val selectionEndY: Float = 0f,
    val selectionBoundsMinX: Float = 0f,
    val selectionBoundsMinY: Float = 0f,
    val selectionBoundsMaxX: Float = 0f,
    val selectionBoundsMaxY: Float = 0f,

    val caretScreenX: Float = 0f,
    val caretScreenY: Float = 0f,
    val caretHeight: Float = 20f
) {
    companion object {
        fun fromJson(jsonStr: String): EditorUiState {
            if (jsonStr.isBlank() || jsonStr == "{}") return EditorUiState()
            return try {
                val obj = JSONObject(jsonStr)
                EditorUiState(
                    isBold = obj.optBoolean("is_bold", false),
                    isItalic = obj.optBoolean("is_italic", false),
                    isUnderline = obj.optBoolean("is_underline", false),
                    isStrikethrough = obj.optBoolean("is_strikethrough", false),
                    alignment = obj.optInt("alignment", 0),
                    styleName = obj.optString("style_name", "Normal"),
                    fontName = obj.optString("font_name", "Calibri"),
                    fontSizePt = obj.optDouble("font_size_pt", 11.0).toFloat(),
                    canUndo = obj.optBoolean("can_undo", false),
                    canRedo = obj.optBoolean("can_redo", false),
                    isDirty = obj.optBoolean("is_dirty", false),
                    pageCount = obj.optInt("page_count", 1),
                    wordCount = obj.optInt("word_count", 0),
                    hasSelection = obj.optBoolean("has_selection", false),

                    selectionStartX = obj.optDouble("selection_start_x", 0.0).toFloat(),
                    selectionStartY = obj.optDouble("selection_start_y", 0.0).toFloat(),
                    selectionEndX = obj.optDouble("selection_end_x", 0.0).toFloat(),
                    selectionEndY = obj.optDouble("selection_end_y", 0.0).toFloat(),
                    selectionBoundsMinX = obj.optDouble("selection_bounds_min_x", 0.0).toFloat(),
                    selectionBoundsMinY = obj.optDouble("selection_bounds_min_y", 0.0).toFloat(),
                    selectionBoundsMaxX = obj.optDouble("selection_bounds_max_x", 0.0).toFloat(),
                    selectionBoundsMaxY = obj.optDouble("selection_bounds_max_y", 0.0).toFloat(),

                    caretScreenX = obj.optDouble("caret_screen_x", 0.0).toFloat(),
                    caretScreenY = obj.optDouble("caret_screen_y", 0.0).toFloat(),
                    caretHeight = obj.optDouble("caret_height", 20.0).toFloat()
                )
            } catch (e: Exception) {
                EditorUiState()
            }
        }
    }
}

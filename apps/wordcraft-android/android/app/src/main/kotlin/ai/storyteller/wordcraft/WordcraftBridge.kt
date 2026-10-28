package ai.storyteller.wordcraft

import android.view.Surface
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONObject

/**
 * High-performance JNI bridge connecting Android Kotlin/Compose to WordCraft Rust Core.
 *
 * Design:
 * - High-frequency hot path: raw primitives for touch, pinch-zoom, fling, and IME.
 * - Reactive state: updates [uiState] StateFlow to drive Compose UI.
 * - Interaction state: [isInteracting] suppresses floating toolbars during scrolls/flings/zooms.
 */
class WordcraftBridge(sample: Boolean = false) {

    private var nativePtr: Long = 0L

    private val _uiState = MutableStateFlow(EditorUiState())
    val uiState: StateFlow<EditorUiState> = _uiState.asStateFlow()

    private val _isInteracting = MutableStateFlow(false)
    val isInteracting: StateFlow<Boolean> = _isInteracting.asStateFlow()

    init {
        nativePtr = nativeInit(sample)
        syncUiState()
    }

    fun destroy() {
        if (nativePtr != 0L) {
            nativeDestroy(nativePtr)
            nativePtr = 0L
        }
    }

    fun setInteracting(interacting: Boolean) {
        _isInteracting.value = interacting
    }

    fun syncUiState() {
        if (nativePtr == 0L) return
        val jsonStr = nativeGetUiStateJson(nativePtr)
        _uiState.value = EditorUiState.fromJson(jsonStr)
    }

    // -------------------------------------------------------------------------
    // Document I/O (Scoped Storage)
    // -------------------------------------------------------------------------

    fun openDocument(filename: String, bytes: ByteArray, password: String? = null): Boolean {
        if (nativePtr == 0L) return false
        val ok = nativeOpenDocument(nativePtr, filename, bytes, password)
        if (ok) syncUiState()
        return ok
    }

    fun saveDocument(filename: String, password: String? = null): ByteArray? {
        if (nativePtr == 0L) return null
        return nativeSaveDocument(nativePtr, filename, password)
    }

    fun exportPdf(tagged: Boolean = true, markup: Boolean = false, comments: Boolean = false): ByteArray? {
        if (nativePtr == 0L) return null
        return nativeExportPdf(nativePtr, tagged, markup, comments)
    }

    // -------------------------------------------------------------------------
    // Command Dispatch
    // -------------------------------------------------------------------------

    fun dispatchCommand(commandId: String, params: Map<String, Any> = emptyMap()): String {
        if (nativePtr == 0L) return """{"status":"error","error":"Bridge destroyed"}"""
        val paramsJson = if (params.isEmpty()) "{}" else JSONObject(params).toString()
        val res = nativeDispatchCommand(nativePtr, commandId, paramsJson)
        syncUiState()
        return res
    }

    // -------------------------------------------------------------------------
    // Surface & Rendering
    // -------------------------------------------------------------------------

    fun attachSurface(surface: Surface, width: Int, height: Int): Boolean {
        if (nativePtr == 0L) return false
        return nativeAttachSurface(nativePtr, surface, width, height)
    }

    fun detachSurface() {
        if (nativePtr != 0L) {
            nativeDetachSurface(nativePtr)
        }
    }

    fun resizeSurface(width: Int, height: Int) {
        if (nativePtr != 0L) {
            nativeResizeSurface(nativePtr, width, height)
            syncUiState()
        }
    }

    fun renderFrame() {
        if (nativePtr != 0L) {
            nativeRenderFrame(nativePtr)
        }
    }

    // -------------------------------------------------------------------------
    // High-Frequency Touch & Gestures
    // -------------------------------------------------------------------------

    fun onTouch(action: Int, pointerId: Int, x: Float, y: Float): Boolean {
        if (nativePtr == 0L) return false
        val handled = nativeOnTouch(nativePtr, action, pointerId, x, y)
        syncUiState()
        return handled
    }

    fun onScroll(dx: Float, dy: Float) {
        if (nativePtr != 0L) {
            nativeOnScroll(nativePtr, dx, dy)
            syncUiState()
        }
    }

    fun onScale(factor: Float, focusX: Float, focusY: Float) {
        if (nativePtr != 0L) {
            nativeOnScale(nativePtr, factor, focusX, focusY)
            syncUiState()
        }
    }

    fun onFling(vx: Float, vy: Float) {
        if (nativePtr != 0L) {
            nativeOnFling(nativePtr, vx, vy)
        }
    }

    fun onDragSelectionHandle(handleRole: Int, x: Float, y: Float): Int {
        if (nativePtr == 0L) return 0
        val result = nativeOnDragSelectionHandle(nativePtr, handleRole, x, y)
        if (result > 0) syncUiState()
        return result
    }

    // -------------------------------------------------------------------------
    // IME & Insets
    // -------------------------------------------------------------------------

    fun imeCommitText(text: String, newCursorPosition: Int) {
        if (nativePtr != 0L) {
            nativeImeCommitText(nativePtr, text, newCursorPosition)
            syncUiState()
        }
    }

    fun imeSetComposingText(text: String, newCursorPosition: Int) {
        if (nativePtr != 0L) {
            nativeImeSetComposingText(nativePtr, text, newCursorPosition)
            syncUiState()
        }
    }

    fun imeFinishComposingText() {
        if (nativePtr != 0L) {
            nativeImeFinishComposingText(nativePtr)
            syncUiState()
        }
    }

    fun imeDeleteSurroundingText(before: Int, after: Int) {
        if (nativePtr != 0L) {
            nativeImeDeleteSurroundingText(nativePtr, before, after)
            syncUiState()
        }
    }

    fun imeBeginBatchEdit() {
        if (nativePtr != 0L) {
            nativeImeBeginBatchEdit(nativePtr)
        }
    }

    fun imeEndBatchEdit() {
        if (nativePtr != 0L) {
            nativeImeEndBatchEdit(nativePtr)
            syncUiState()
        }
    }

    fun imeGetTextBeforeCursor(n: Int): String {
        return if (nativePtr != 0L) nativeImeGetTextBeforeCursor(nativePtr, n) else ""
    }

    fun imeGetTextAfterCursor(n: Int): String {
        return if (nativePtr != 0L) nativeImeGetTextAfterCursor(nativePtr, n) else ""
    }

    fun imeGetSelectedText(): String {
        return if (nativePtr != 0L) nativeImeGetSelectedText(nativePtr) else ""
    }

    fun updateInsets(statusBar: Float, navBar: Float, imeHeight: Float, screenWidth: Float, screenHeight: Float) {
        if (nativePtr != 0L) {
            nativeUpdateInsets(nativePtr, statusBar, navBar, imeHeight, screenWidth, screenHeight)
            syncUiState()
        }
    }

    fun ensureCaretVisible(): Boolean {
        if (nativePtr == 0L) return false
        val changed = nativeEnsureCaretVisible(nativePtr)
        if (changed) syncUiState()
        return changed
    }

    // -------------------------------------------------------------------------
    // External Native Declarations (Matches jni_bridge.rs 1:1)
    // -------------------------------------------------------------------------

    private external fun nativeInit(sample: Boolean): Long
    private external fun nativeDestroy(ptr: Long)
    private external fun nativeOpenDocument(ptr: Long, filename: String, bytes: ByteArray, password: String?): Boolean
    private external fun nativeSaveDocument(ptr: Long, filename: String, password: String?): ByteArray?
    private external fun nativeExportPdf(ptr: Long, tagged: Boolean, markup: Boolean, comments: Boolean): ByteArray?
    private external fun nativeDispatchCommand(ptr: Long, cmdId: String, paramsJson: String): String

    private external fun nativeAttachSurface(ptr: Long, surface: Surface, width: Int, height: Int): Boolean
    private external fun nativeDetachSurface(ptr: Long)
    private external fun nativeResizeSurface(ptr: Long, width: Int, height: Int)
    private external fun nativeRenderFrame(ptr: Long)

    private external fun nativeOnTouch(ptr: Long, action: Int, pointerId: Int, x: Float, y: Float): Boolean
    private external fun nativeOnScroll(ptr: Long, dx: Float, dy: Float)
    private external fun nativeOnScale(ptr: Long, factor: Float, focusX: Float, focusY: Float)
    private external fun nativeOnFling(ptr: Long, vx: Float, vy: Float)
    private external fun nativeOnDragSelectionHandle(ptr: Long, handleRole: Int, x: Float, y: Float): Int

    private external fun nativeImeCommitText(ptr: Long, text: String, newCursorPos: Int)
    private external fun nativeImeSetComposingText(ptr: Long, text: String, newCursorPos: Int)
    private external fun nativeImeFinishComposingText(ptr: Long)
    private external fun nativeImeDeleteSurroundingText(ptr: Long, before: Int, after: Int)
    private external fun nativeImeBeginBatchEdit(ptr: Long)
    private external fun nativeImeEndBatchEdit(ptr: Long)
    private external fun nativeImeGetTextBeforeCursor(ptr: Long, n: Int): String
    private external fun nativeImeGetTextAfterCursor(ptr: Long, n: Int): String
    private external fun nativeImeGetSelectedText(ptr: Long): String

    private external fun nativeUpdateInsets(ptr: Long, statusBar: Float, navBar: Float, imeHeight: Float, screenWidth: Float, screenHeight: Float)
    private external fun nativeEnsureCaretVisible(ptr: Long): Boolean

    private external fun nativeGetUiStateJson(ptr: Long): String

    companion object {
        init {
            System.loadLibrary("wordcraft_android")
        }
    }
}

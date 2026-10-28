package ai.storyteller.wordcraft

import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection

/**
 * Custom InputConnection for WordCraft, bridging native Android keyboard events
 * (Gboard swipe-typing, autocomplete suggestions, voice input, and batch undo steps)
 * directly into the Rust engine.
 */
class WordcraftInputConnection(
    targetView: View,
    private val bridge: WordcraftBridge
) : BaseInputConnection(targetView, true) {

    private var batchDepth = 0

    override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence {
        return bridge.imeGetTextBeforeCursor(n)
    }

    override fun getTextAfterCursor(n: Int, flags: Int): CharSequence {
        return bridge.imeGetTextAfterCursor(n)
    }

    override fun getSelectedText(flags: Int): CharSequence {
        return bridge.imeGetSelectedText()
    }

    override fun beginBatchEdit(): Boolean {
        batchDepth++
        if (batchDepth == 1) {
            bridge.imeBeginBatchEdit()
        }
        return true
    }

    override fun endBatchEdit(): Boolean {
        if (batchDepth > 0) {
            batchDepth--
            if (batchDepth == 0) {
                bridge.imeEndBatchEdit()
            }
        }
        return true
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
        if (!text.isNullOrEmpty()) {
            bridge.imeCommitText(text.toString(), newCursorPosition)
        }
        return true
    }

    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
        if (text != null) {
            bridge.imeSetComposingText(text.toString(), newCursorPosition)
        }
        return true
    }

    override fun finishComposingText(): Boolean {
        bridge.imeFinishComposingText()
        return true
    }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
        if (beforeLength > 0 || afterLength > 0) {
            bridge.imeDeleteSurroundingText(beforeLength, afterLength)
        }
        return true
    }

    override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean {
        // Delegate code point deletions to character deletions
        return deleteSurroundingText(beforeLength, afterLength)
    }

    override fun sendKeyEvent(event: KeyEvent): Boolean {
        if (event.action == KeyEvent.ACTION_DOWN) {
            val isCtrl = event.isCtrlPressed
            val isShift = event.isShiftPressed

            // 1. Physical Keyboard Shortcuts (Ctrl / Cmd + Key)
            if (isCtrl) {
                when (event.keyCode) {
                    KeyEvent.KEYCODE_Z -> {
                        if (isShift) {
                            bridge.dispatchCommand("edit.redo")
                        } else {
                            bridge.dispatchCommand("edit.undo")
                        }
                        return true
                    }
                    KeyEvent.KEYCODE_Y -> {
                        bridge.dispatchCommand("edit.redo")
                        return true
                    }
                    KeyEvent.KEYCODE_B -> {
                        bridge.dispatchCommand("format.bold")
                        return true
                    }
                    KeyEvent.KEYCODE_I -> {
                        bridge.dispatchCommand("format.italic")
                        return true
                    }
                    KeyEvent.KEYCODE_U -> {
                        bridge.dispatchCommand("format.underline")
                        return true
                    }
                    KeyEvent.KEYCODE_A -> {
                        bridge.dispatchCommand("select.all")
                        return true
                    }
                    KeyEvent.KEYCODE_C -> {
                        bridge.dispatchCommand("edit.copy")
                        return true
                    }
                    KeyEvent.KEYCODE_X -> {
                        bridge.dispatchCommand("edit.cut")
                        return true
                    }
                    KeyEvent.KEYCODE_V -> {
                        bridge.dispatchCommand("edit.paste")
                        return true
                    }
                    KeyEvent.KEYCODE_DEL -> {
                        bridge.dispatchCommand("text.deleteWordBack")
                        return true
                    }
                    KeyEvent.KEYCODE_FORWARD_DEL -> {
                        bridge.dispatchCommand("text.deleteWordForward")
                        return true
                    }
                    KeyEvent.KEYCODE_ENTER -> {
                        bridge.dispatchCommand("text.pageBreak")
                        return true
                    }
                    KeyEvent.KEYCODE_DPAD_LEFT -> {
                        if (isShift) {
                            bridge.dispatchCommand("select.wordLeft")
                        } else {
                            bridge.dispatchCommand("caret.wordLeft")
                        }
                        return true
                    }
                    KeyEvent.KEYCODE_DPAD_RIGHT -> {
                        if (isShift) {
                            bridge.dispatchCommand("select.wordRight")
                        } else {
                            bridge.dispatchCommand("caret.wordRight")
                        }
                        return true
                    }
                }
            }

            // 2. Standard Key Events
            when (event.keyCode) {
                KeyEvent.KEYCODE_DEL -> {
                    bridge.imeDeleteSurroundingText(1, 0)
                    return true
                }
                KeyEvent.KEYCODE_FORWARD_DEL -> {
                    bridge.dispatchCommand("text.delete")
                    return true
                }
                KeyEvent.KEYCODE_ENTER -> {
                    if (isShift) {
                        bridge.dispatchCommand("text.lineBreak")
                    } else {
                        bridge.dispatchCommand("text.newParagraph")
                    }
                    return true
                }
                KeyEvent.KEYCODE_TAB -> {
                    if (isShift) {
                        bridge.dispatchCommand("text.backTab")
                    } else {
                        bridge.dispatchCommand("text.tab")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_LEFT -> {
                    if (isShift) {
                        bridge.dispatchCommand("select.left")
                    } else {
                        bridge.dispatchCommand("caret.left")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_RIGHT -> {
                    if (isShift) {
                        bridge.dispatchCommand("select.right")
                    } else {
                        bridge.dispatchCommand("caret.right")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_UP -> {
                    if (isShift) {
                        bridge.dispatchCommand("select.up")
                    } else {
                        bridge.dispatchCommand("caret.up")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_DOWN -> {
                    if (isShift) {
                        bridge.dispatchCommand("select.down")
                    } else {
                        bridge.dispatchCommand("caret.down")
                    }
                    return true
                }
            }
        }
        return super.sendKeyEvent(event)
    }
}

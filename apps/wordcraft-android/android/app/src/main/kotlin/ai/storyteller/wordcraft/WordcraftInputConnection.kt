package ai.storyteller.wordcraft

import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.InputConnection

/**
 * Custom InputConnection for WordCraft, bridging native Android keyboard events
 * (Gboard swipe-typing, autocomplete suggestions, voice input, and batch undo steps)
 * directly into the Rust engine.
 */
class WordcraftInputConnection(
    targetView: View
) : BaseInputConnection(targetView, true) {

    private var batchDepth = 0

    override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence {
        return WordcraftBridge.nativeImeGetTextBeforeCursor(n)
    }

    override fun getTextAfterCursor(n: Int, flags: Int): CharSequence {
        return WordcraftBridge.nativeImeGetTextAfterCursor(n)
    }

    override fun getSelectedText(flags: Int): CharSequence {
        return WordcraftBridge.nativeImeGetSelectedText()
    }

    override fun beginBatchEdit(): Boolean {
        batchDepth++
        if (batchDepth == 1) {
            WordcraftBridge.nativeImeBeginBatchEdit()
        }
        return true
    }

    override fun endBatchEdit(): Boolean {
        if (batchDepth > 0) {
            batchDepth--
            if (batchDepth == 0) {
                WordcraftBridge.nativeImeEndBatchEdit()
            }
        }
        return true
    }

    override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
        if (!text.isNullOrEmpty()) {
            WordcraftBridge.nativeImeCommitText(text.toString())
        }
        return true
    }

    override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
        if (text != null) {
            WordcraftBridge.nativeImeSetComposingText(text.toString(), newCursorPosition)
        }
        return true
    }

    override fun finishComposingText(): Boolean {
        WordcraftBridge.nativeImeFinishComposingText()
        return true
    }

    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
        if (beforeLength > 0 || afterLength > 0) {
            WordcraftBridge.nativeImeDeleteSurroundingText(beforeLength, afterLength)
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
                            WordcraftBridge.dispatchCommand("edit.redo")
                        } else {
                            WordcraftBridge.dispatchCommand("edit.undo")
                        }
                        return true
                    }
                    KeyEvent.KEYCODE_Y -> {
                        WordcraftBridge.dispatchCommand("edit.redo")
                        return true
                    }
                    KeyEvent.KEYCODE_B -> {
                        WordcraftBridge.dispatchCommand("format.bold")
                        return true
                    }
                    KeyEvent.KEYCODE_I -> {
                        WordcraftBridge.dispatchCommand("format.italic")
                        return true
                    }
                    KeyEvent.KEYCODE_U -> {
                        WordcraftBridge.dispatchCommand("format.underline")
                        return true
                    }
                    KeyEvent.KEYCODE_A -> {
                        WordcraftBridge.dispatchCommand("select.all")
                        return true
                    }
                    KeyEvent.KEYCODE_C -> {
                        WordcraftBridge.dispatchCommand("edit.copy")
                        return true
                    }
                    KeyEvent.KEYCODE_X -> {
                        WordcraftBridge.dispatchCommand("edit.cut")
                        return true
                    }
                    KeyEvent.KEYCODE_V -> {
                        WordcraftBridge.dispatchCommand("edit.paste")
                        return true
                    }
                    KeyEvent.KEYCODE_DEL -> {
                        WordcraftBridge.dispatchCommand("text.deleteWordBack")
                        return true
                    }
                    KeyEvent.KEYCODE_FORWARD_DEL -> {
                        WordcraftBridge.dispatchCommand("text.deleteWordForward")
                        return true
                    }
                    KeyEvent.KEYCODE_ENTER -> {
                        WordcraftBridge.dispatchCommand("text.pageBreak")
                        return true
                    }
                    KeyEvent.KEYCODE_DPAD_LEFT -> {
                        if (isShift) {
                            WordcraftBridge.dispatchCommand("select.wordLeft")
                        } else {
                            WordcraftBridge.dispatchCommand("caret.wordLeft")
                        }
                        return true
                    }
                    KeyEvent.KEYCODE_DPAD_RIGHT -> {
                        if (isShift) {
                            WordcraftBridge.dispatchCommand("select.wordRight")
                        } else {
                            WordcraftBridge.dispatchCommand("caret.wordRight")
                        }
                        return true
                    }
                }
            }

            // 2. Standard Key Events
            when (event.keyCode) {
                KeyEvent.KEYCODE_DEL -> {
                    WordcraftBridge.nativeImeDeleteSurroundingText(1, 0)
                    return true
                }
                KeyEvent.KEYCODE_FORWARD_DEL -> {
                    WordcraftBridge.dispatchCommand("text.delete")
                    return true
                }
                KeyEvent.KEYCODE_ENTER -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("text.lineBreak")
                    } else {
                        WordcraftBridge.dispatchCommand("text.newParagraph")
                    }
                    return true
                }
                KeyEvent.KEYCODE_TAB -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("text.backTab")
                    } else {
                        WordcraftBridge.dispatchCommand("text.tab")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_LEFT -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("select.left")
                    } else {
                        WordcraftBridge.dispatchCommand("caret.left")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_RIGHT -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("select.right")
                    } else {
                        WordcraftBridge.dispatchCommand("caret.right")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_UP -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("select.up")
                    } else {
                        WordcraftBridge.dispatchCommand("caret.up")
                    }
                    return true
                }
                KeyEvent.KEYCODE_DPAD_DOWN -> {
                    if (isShift) {
                        WordcraftBridge.dispatchCommand("select.down")
                    } else {
                        WordcraftBridge.dispatchCommand("caret.down")
                    }
                    return true
                }
            }
        }
        return super.sendKeyEvent(event)
    }
}

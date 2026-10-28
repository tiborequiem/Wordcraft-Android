package ai.storyteller.wordcraft

import android.content.Context
import android.graphics.PixelFormat
import android.view.Choreographer
import android.view.GestureDetector
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.OverScroller

/**
 * Android SurfaceView hosting WordCraft's Rust WGPU canvas renderer.
 *
 * Provides:
 * - Surface lifecycle management (attach, resize, detach) via ANativeWindow.
 * - Hardware 60/120fps render loop via Choreographer.
 * - Native OverScroller momentum / fling physics with spline deceleration.
 * - Pinch-to-zoom via ScaleGestureDetector clamped [0.5x - 4.0x].
 * - Full IME soft keyboard integration via [WordcraftInputConnection].
 * - Haptic feedback on selection / long-press.
 */
class WordcraftCanvas(
    context: Context,
    val bridge: WordcraftBridge
) : SurfaceView(context), SurfaceHolder.Callback, Choreographer.FrameCallback {

    private val overScroller = OverScroller(context)
    private var lastFlingX = 0
    private var lastFlingY = 0
    private var isFlinging = false

    private val choreographer = Choreographer.getInstance()
    private var isRendering = false

    private val scaleDetector: ScaleGestureDetector
    private val gestureDetector: GestureDetector

    private var inputConnection: WordcraftInputConnection? = null

    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.TRANSLUCENT)

        isFocusable = true
        isFocusableInTouchMode = true

        scaleDetector = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                bridge.setInteracting(true)
                return true
            }

            override fun onScale(detector: ScaleGestureDetector): Boolean {
                bridge.onScale(detector.scaleFactor, detector.focusX, detector.focusY)
                return true
            }

            override fun onScaleEnd(detector: ScaleGestureDetector) {
                bridge.setInteracting(false)
            }
        })

        gestureDetector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean {
                if (isFlinging) {
                    overScroller.abortAnimation()
                    isFlinging = false
                    bridge.setInteracting(false)
                }
                return true
            }

            override fun onScroll(
                e1: MotionEvent?,
                e2: MotionEvent,
                distanceX: Float,
                distanceY: Float
            ): Boolean {
                bridge.setInteracting(true)
                bridge.onScroll(distanceX, distanceY)
                return true
            }

            override fun onFling(
                e1: MotionEvent?,
                e2: MotionEvent,
                velocityX: Float,
                velocityY: Float
            ): Boolean {
                bridge.setInteracting(true)
                isFlinging = true
                lastFlingX = 0
                lastFlingY = 0

                overScroller.fling(
                    0, 0,
                    velocityX.toInt(), velocityY.toInt(),
                    Int.MIN_VALUE, Int.MAX_VALUE,
                    Int.MIN_VALUE, Int.MAX_VALUE
                )
                return true
            }

            override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                requestFocus()
                showSoftKeyboard()
                bridge.onTouch(MotionEvent.ACTION_DOWN, e.getPointerId(0), e.x, e.y)
                bridge.onTouch(MotionEvent.ACTION_UP, e.getPointerId(0), e.x, e.y)
                return true
            }

            override fun onDoubleTap(e: MotionEvent): Boolean {
                performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                bridge.dispatchCommand("select.word")
                return true
            }

            override fun onLongPress(e: MotionEvent) {
                performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                bridge.onTouch(MotionEvent.ACTION_DOWN, e.getPointerId(0), e.x, e.y)
                bridge.dispatchCommand("select.word")
            }
        })
    }

    // -------------------------------------------------------------------------
    // SurfaceHolder.Callback Lifecycle
    // -------------------------------------------------------------------------

    override fun surfaceCreated(holder: SurfaceHolder) {
        bridge.attachSurface(holder.surface, width, height)
        startRenderLoop()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        bridge.resizeSurface(width, height)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopRenderLoop()
        bridge.detachSurface()
    }

    // -------------------------------------------------------------------------
    // Choreographer 60/120fps Render Loop & Spline Fling
    // -------------------------------------------------------------------------

    private fun startRenderLoop() {
        if (!isRendering) {
            isRendering = true
            choreographer.postFrameCallback(this)
        }
    }

    private fun stopRenderLoop() {
        isRendering = false
        choreographer.removeFrameCallback(this)
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!isRendering) return

        // 1. Compute fling momentum physics step
        if (isFlinging && overScroller.computeScrollOffset()) {
            val currX = overScroller.currX
            val currY = overScroller.currY
            val dx = (currX - lastFlingX).toFloat()
            val dy = (currY - lastFlingY).toFloat()
            lastFlingX = currX
            lastFlingY = currY

            bridge.onScroll(-dx, -dy)

            if (overScroller.isFinished) {
                isFlinging = false
                bridge.setInteracting(false)
            }
        } else if (isFlinging && overScroller.isFinished) {
            isFlinging = false
            bridge.setInteracting(false)
        }

        // 2. Render WGPU frame
        bridge.renderFrame()

        // 3. Schedule next vsync tick
        choreographer.postFrameCallback(this)
    }

    // -------------------------------------------------------------------------
    // MotionEvent Handling
    // -------------------------------------------------------------------------

    override fun onTouchEvent(event: MotionEvent): Boolean {
        var handled = scaleDetector.onTouchEvent(event)
        handled = gestureDetector.onTouchEvent(event) || handled

        val action = event.actionMasked
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            if (!isFlinging) {
                bridge.setInteracting(false)
            }
        }

        return handled || super.onTouchEvent(event)
    }

    // -------------------------------------------------------------------------
    // Soft Keyboard (IME) Integration
    // -------------------------------------------------------------------------

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = EditorInfo.TYPE_CLASS_TEXT or
                EditorInfo.TYPE_TEXT_FLAG_MULTI_LINE or
                EditorInfo.TYPE_TEXT_FLAG_AUTO_CORRECT
        outAttrs.imeOptions = EditorInfo.IME_ACTION_NONE or EditorInfo.IME_FLAG_NO_FULLSCREEN

        val conn = WordcraftInputConnection(this, bridge)
        inputConnection = conn
        return conn
    }

    fun showSoftKeyboard() {
        val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        imm?.showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
    }

    fun hideSoftKeyboard() {
        val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        imm?.hideSoftInputFromWindow(windowToken, 0)
    }
}

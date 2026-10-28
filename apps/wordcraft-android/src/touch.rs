//! Touch gesture state machine & text selection mechanics for WordCraft on Android.
//!
//! Provides:
//! - Single-finger kinetic drag & momentum/fling scrolling.
//! - Two-finger pinch-to-zoom viewport scaling.
//! - Tap detection (caret placement, soft keyboard request).
//! - Double-tap & long-press word selection.
//! - Teardrop selection handle calculation (start and end).
//! - Floating selection toolbar anchor rect calculation.

use std::time::Instant;

/// Action codes matching Android's `MotionEvent.ACTION_*`.
pub const ACTION_DOWN: i32 = 0;
pub const ACTION_UP: i32 = 1;
pub const ACTION_MOVE: i32 = 2;
pub const ACTION_CANCEL: i32 = 3;
pub const ACTION_POINTER_DOWN: i32 = 5;
pub const ACTION_POINTER_UP: i32 = 6;

/// Touch gesture thresholds.
const TAP_TIMEOUT_MS: u128 = 250;
const LONG_PRESS_TIMEOUT_MS: u128 = 500;
const TAP_SLOP_PX: f32 = 24.0;
const DOUBLE_TAP_TIMEOUT_MS: u128 = 300;
const DOUBLE_TAP_SLOP_PX: f32 = 40.0;

/// Momentum scrolling parameters.
const VELOCITY_HISTORY_SIZE: usize = 5;
const FLING_FRICTION: f32 = 0.94;
const MIN_FLING_VELOCITY: f32 = 60.0; // px/sec

/// Geometry of a draggable teardrop selection handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub is_start: bool,
}

/// Floating selection toolbar configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingToolbar {
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub items: Vec<ToolbarAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarAction {
    Cut,
    Copy,
    Paste,
    Comment,
    Highlight,
}

/// Event emitted by the touch gesture recognizer.
#[derive(Debug, PartialEq)]
pub enum GestureEvent {
    None,
    /// Single-finger scroll delta (dx, dy).
    Scroll { dx: f32, dy: f32 },
    /// Pinch-to-zoom: scale multiplier and focus point.
    Zoom { scale_factor: f32, focus_x: f32, focus_y: f32 },
    /// Single tap at screen coordinate (x, y) - request soft keyboard.
    Tap { x: f32, y: f32 },
    /// Double tap at (x, y) - select word & trigger haptics.
    DoubleTap { x: f32, y: f32 },
    /// Long press at (x, y) - select word & trigger haptics.
    LongPress { x: f32, y: f32 },
    /// Dragging a selection handle.
    DragHandle { is_start: bool, x: f32, y: f32 },
}

/// Motion state of a touch pointer.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct PointerState {
    id: i32,
    x: f32,
    y: f32,
}

/// State tracking for kinetic momentum flings.
#[derive(Clone, Debug, Default)]
pub struct FlingState {
    pub vx: f32,
    pub vy: f32,
    pub is_active: bool,
}

impl FlingState {
    pub fn step(&mut self, dt_sec: f32) -> Option<(f32, f32)> {
        if !self.is_active {
            return None;
        }
        let decay = FLING_FRICTION.powf(dt_sec * 60.0);
        self.vx *= decay;
        self.vy *= decay;

        if self.vx.abs() < MIN_FLING_VELOCITY && self.vy.abs() < MIN_FLING_VELOCITY {
            self.is_active = false;
            return None;
        }

        Some((self.vx * dt_sec, self.vy * dt_sec))
    }
}

/// Main touch gesture recognizer for WordCraft.
pub struct TouchRecognizer {
    active_pointers: Vec<PointerState>,
    down_time: Option<Instant>,
    down_pos: (f32, f32),
    last_pos: (f32, f32),
    last_tap_time: Option<Instant>,
    last_tap_pos: (f32, f32),
    is_dragging: bool,
    is_pinching: bool,
    initial_pinch_dist: f32,
    long_press_triggered: bool,

    // Velocity history for fling calculation
    velocity_history: Vec<(Instant, f32, f32)>,

    // Active dragging of selection handles
    pub dragging_handle: Option<bool>, // true = start, false = end
    pub fling: FlingState,
}

impl Default for TouchRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchRecognizer {
    pub fn new() -> Self {
        Self {
            active_pointers: Vec::with_capacity(4),
            down_time: None,
            down_pos: (0.0, 0.0),
            last_pos: (0.0, 0.0),
            last_tap_time: None,
            last_tap_pos: (0.0, 0.0),
            is_dragging: false,
            is_pinching: false,
            initial_pinch_dist: 1.0,
            long_press_triggered: false,
            velocity_history: Vec::with_capacity(VELOCITY_HISTORY_SIZE),
            dragging_handle: None,
            fling: FlingState::default(),
        }
    }

    /// Process a touch event from Android JNI or winit.
    pub fn on_touch(
        &mut self,
        action: i32,
        pointers: &[(i32, f32, f32)],
        start_handle: Option<SelectionHandle>,
        end_handle: Option<SelectionHandle>,
    ) -> GestureEvent {
        let now = Instant::now();
        let action_masked = action & 0xFF;

        match action_masked {
            ACTION_DOWN => {
                if let Some(&(_, x, y)) = pointers.first() {
                    self.active_pointers.clear();
                    self.active_pointers.push(PointerState { id: 0, x, y });
                    self.down_time = Some(now);
                    self.down_pos = (x, y);
                    self.last_pos = (x, y);
                    self.is_dragging = false;
                    self.is_pinching = false;
                    self.long_press_triggered = false;
                    self.fling.is_active = false;
                    self.velocity_history.clear();
                    self.velocity_history.push((now, x, y));

                    // Check if pointer hit a selection handle
                    if let Some(sh) = start_handle {
                        if (x - sh.x).abs() <= sh.width && (y - sh.y).abs() <= sh.height {
                            self.dragging_handle = Some(true);
                            return GestureEvent::None;
                        }
                    }
                    if let Some(eh) = end_handle {
                        if (x - eh.x).abs() <= eh.width && (y - eh.y).abs() <= eh.height {
                            self.dragging_handle = Some(false);
                            return GestureEvent::None;
                        }
                    }
                    self.dragging_handle = None;
                }
                GestureEvent::None
            }

            ACTION_POINTER_DOWN => {
                // Multi-touch: transition to pinch-to-zoom
                if pointers.len() >= 2 {
                    let p1 = pointers[0];
                    let p2 = pointers[1];
                    let dx = p2.1 - p1.1;
                    let dy = p2.2 - p1.2;
                    self.initial_pinch_dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    self.is_pinching = true;
                    self.is_dragging = false;
                    self.dragging_handle = None;
                }
                GestureEvent::None
            }

            ACTION_MOVE => {
                if let Some(is_start) = self.dragging_handle {
                    if let Some(&(_, x, y)) = pointers.first() {
                        return GestureEvent::DragHandle { is_start, x, y };
                    }
                }

                if self.is_pinching && pointers.len() >= 2 {
                    let p1 = pointers[0];
                    let p2 = pointers[1];
                    let dx = p2.1 - p1.1;
                    let dy = p2.2 - p1.2;
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let scale_factor = dist / self.initial_pinch_dist;
                    self.initial_pinch_dist = dist;
                    let focus_x = (p1.1 + p2.1) * 0.5;
                    let focus_y = (p1.2 + p2.2) * 0.5;
                    return GestureEvent::Zoom { scale_factor, focus_x, focus_y };
                }

                if let Some(&(_, x, y)) = pointers.first() {
                    let dx = x - self.last_pos.0;
                    let dy = y - self.last_pos.1;
                    let total_dist = ((x - self.down_pos.0).powi(2) + (y - self.down_pos.1).powi(2)).sqrt();

                    // Check for long-press trigger if user hasn't moved much
                    if !self.is_dragging && !self.long_press_triggered {
                        if let Some(dt) = self.down_time {
                            if now.duration_since(dt).as_millis() >= LONG_PRESS_TIMEOUT_MS && total_dist < TAP_SLOP_PX {
                                self.long_press_triggered = true;
                                return GestureEvent::LongPress { x, y };
                            }
                        }
                    }

                    if total_dist > TAP_SLOP_PX {
                        self.is_dragging = true;
                    }

                    if self.is_dragging {
                        self.last_pos = (x, y);
                        self.record_velocity(now, x, y);
                        return GestureEvent::Scroll { dx, dy };
                    }
                }
                GestureEvent::None
            }

            ACTION_UP => {
                self.dragging_handle = None;
                if self.is_pinching {
                    self.is_pinching = false;
                    return GestureEvent::None;
                }

                if let Some(&(_, x, y)) = pointers.first() {
                    let total_dist = ((x - self.down_pos.0).powi(2) + (y - self.down_pos.1).powi(2)).sqrt();
                    let duration = self.down_time.map_or(999, |t| now.duration_since(t).as_millis());

                    // Check if tap or double-tap
                    if !self.is_dragging && !self.long_press_triggered && total_dist < TAP_SLOP_PX && duration < TAP_TIMEOUT_MS {
                        let is_double_tap = if let Some(last_t) = self.last_tap_time {
                            let tap_interval = now.duration_since(last_t).as_millis();
                            let tap_dist = ((x - self.last_tap_pos.0).powi(2) + (y - self.last_tap_pos.1).powi(2)).sqrt();
                            tap_interval < DOUBLE_TAP_TIMEOUT_MS && tap_dist < DOUBLE_TAP_SLOP_PX
                        } else {
                            false
                        };

                        self.last_tap_time = Some(now);
                        self.last_tap_pos = (x, y);

                        if is_double_tap {
                            self.last_tap_time = None;
                            return GestureEvent::DoubleTap { x, y };
                        } else {
                            return GestureEvent::Tap { x, y };
                        }
                    }

                    // Compute fling velocity
                    if self.is_dragging {
                        self.calculate_fling();
                    }
                }
                GestureEvent::None
            }

            ACTION_POINTER_UP => {
                if pointers.len() <= 2 {
                    self.is_pinching = false;
                }
                GestureEvent::None
            }

            ACTION_CANCEL => {
                self.is_dragging = false;
                self.is_pinching = false;
                self.dragging_handle = None;
                self.fling.is_active = false;
                GestureEvent::None
            }

            _ => GestureEvent::None,
        }
    }

    fn record_velocity(&mut self, time: Instant, x: f32, y: f32) {
        if self.velocity_history.len() >= VELOCITY_HISTORY_SIZE {
            self.velocity_history.remove(0);
        }
        self.velocity_history.push((time, x, y));
    }

    fn calculate_fling(&mut self) {
        if self.velocity_history.len() < 2 {
            return;
        }
        if let (Some(first), Some(last)) = (self.velocity_history.first(), self.velocity_history.last()) {
            let dt = last.0.duration_since(first.0).as_secs_f32();
            if dt > 0.001 && dt < 0.2 {
                let vx = (last.1 - first.1) / dt;
                let vy = (last.2 - first.2) / dt;
                if vx.abs() >= MIN_FLING_VELOCITY || vy.abs() >= MIN_FLING_VELOCITY {
                    self.fling.vx = vx;
                    self.fling.vy = vy;
                    self.fling.is_active = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fling_state_deceleration() {
        let mut fling = FlingState {
            vx: 1000.0,
            vy: 2000.0,
            is_active: true,
        };

        // Step 16ms
        let step1 = fling.step(0.016);
        assert!(step1.is_some());
        let (dx, dy) = step1.unwrap();
        assert!(dx > 0.0);
        assert!(dy > 0.0);
        assert!(fling.vx < 1000.0);
        assert!(fling.vy < 2000.0);

        // Run until fling naturally expires below MIN_FLING_VELOCITY
        for _ in 0..300 {
            fling.step(0.016);
        }
        assert!(!fling.is_active);
        assert_eq!(fling.step(0.016), None);
    }

    #[test]
    fn test_touch_tap_and_double_tap() {
        let mut recognizer = TouchRecognizer::new();

        // Down
        let ev1 = recognizer.on_touch(ACTION_DOWN, &[(0, 100.0, 100.0)], None, None);
        assert_eq!(ev1, GestureEvent::None);

        // Quick Up -> Tap
        let ev2 = recognizer.on_touch(ACTION_UP, &[(0, 100.0, 100.0)], None, None);
        assert_eq!(ev2, GestureEvent::Tap { x: 100.0, y: 100.0 });

        // Second quick Down & Up -> Double Tap
        let _ = recognizer.on_touch(ACTION_DOWN, &[(0, 102.0, 101.0)], None, None);
        let ev4 = recognizer.on_touch(ACTION_UP, &[(0, 102.0, 101.0)], None, None);
        assert_eq!(ev4, GestureEvent::DoubleTap { x: 102.0, y: 101.0 });
    }

    #[test]
    fn test_pinch_zoom_gesture() {
        let mut recognizer = TouchRecognizer::new();

        // Down finger 1
        recognizer.on_touch(ACTION_DOWN, &[(0, 100.0, 100.0)], None, None);

        // Down finger 2 at distance 100
        recognizer.on_touch(ACTION_POINTER_DOWN, &[(0, 100.0, 100.0), (1, 200.0, 100.0)], None, None);

        // Move finger 2 outward to distance 200 (scale 2.0x)
        let ev = recognizer.on_touch(ACTION_MOVE, &[(0, 100.0, 100.0), (1, 300.0, 100.0)], None, None);
        match ev {
            GestureEvent::Zoom { scale_factor, focus_x, focus_y } => {
                assert!(scale_factor > 1.0);
                assert_eq!(focus_x, 200.0);
                assert_eq!(focus_y, 100.0);
            }
            other => panic!("Expected Zoom event, got {other:?}"),
        }
    }

    #[test]
    fn test_selection_handle_detection() {
        let mut recognizer = TouchRecognizer::new();
        let start_handle = SelectionHandle {
            x: 50.0,
            y: 120.0,
            width: 24.0,
            height: 24.0,
            is_start: true,
        };

        // Touch on start handle
        recognizer.on_touch(ACTION_DOWN, &[(0, 52.0, 122.0)], Some(start_handle), None);
        assert_eq!(recognizer.dragging_handle, Some(true));

        // Drag handle
        let ev = recognizer.on_touch(ACTION_MOVE, &[(0, 60.0, 125.0)], Some(start_handle), None);
        assert_eq!(ev, GestureEvent::DragHandle { is_start: true, x: 60.0, y: 125.0 });
    }
}

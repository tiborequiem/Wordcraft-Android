<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/ai.storyteller.wordcraft.png" width="128" height="128" alt="WordCraft Android icon: a document page and pen on a blue tile">
</p>

<h1 align="center">WordCraft for Android</h1>

<p align="center">
  <b>A first-class, touch-optimized native Android port of the pure-Rust WordCraft word processor.</b><br>
  <i>Built with Rust, WGPU (Vulkan & GLES), JNI, and Jetpack Compose Material 3.</i>
</p>

<p align="center">
  <img alt="Platform: Android" src="https://img.shields.io/badge/platform-Android%208.0%2B%20(API%2026%2B)-3DDC84?style=flat-square&logo=android&logoColor=white">
  <img alt="Core: Rust" src="https://img.shields.io/badge/core-Pure%20Rust-2b47b5?style=flat-square&logo=rust&logoColor=white">
  <img alt="UI: Jetpack Compose" src="https://img.shields.io/badge/UI-Jetpack%20Compose%20(Material%203)-4285F4?style=flat-square&logo=jetpackcompose&logoColor=white">
  <img alt="Rendering: WGPU Vulkan/GLES" src="https://img.shields.io/badge/rendering-WGPU%20(Vulkan%20%2F%20GLES)-black?style=flat-square">
  <img alt="Arch: arm64-v8a / x86_64" src="https://img.shields.io/badge/arch-arm64--v8a%20%C2%B7%20x86__64-orange?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-2b47b5?style=flat-square">
</p>

---

## 📱 Overview

**WordCraft Android** brings WordCraft's 389+ Microsoft Word-parity engine commands, line breaking, multi-page layout, and document serialization directly to Android devices. 

Rather than a sluggish WebView or an un-ergonomic desktop-to-mobile compile, WordCraft Android implements a **true native hybrid architecture**:
1. **Rust Core Engine (`wordcraft_engine`)**: Document model, typography, pagination, undo/redo history, and format parsers.
2. **Native Surface View (`wgpu`)**: Hardware-accelerated Vulkan canvas rendering with OpenGL ES fallback on a native `SurfaceView`.
3. **Jetpack Compose Material 3 Chrome**: Native mobile UX including Top App Bar, contextual Keyboard Formatting Bar, animated Floating Selection Toolbar, and Modal Bottom Sheets.
4. **Split JNI Bridge**: High-frequency zero-allocation primitives for 60/120fps touch gestures, pinch-zoom, and soft keyboard typing.

---

## ✨ Features

- **Touch Ergonomics & Momentum Physics**:
  - Kinetic scrolling with native Android `OverScroller` spline deceleration.
  - Smooth pinch-to-zoom viewport scaling clamped `0.5x` to `4.0x`.
  - **Teardrop Selection Handles** with finger-occlusion offset (`y - 36.dp`) tracking the text baseline above the user's thumb.
  - Seamless handle crossover swapping with haptic feedback.
  - Floating `Modifier.magnifier` lens during handle dragging.
  - Floating selection action mode (`Cut`, `Copy`, `Paste`, `Highlight`, `Comment`) that smoothly fades out during scroll/gestures and returns when settled.

- **Soft Keyboard (IME) Integration**:
  - Custom `WordcraftInputConnection` providing full Gboard autocompletion and predictive swipe typing context (`getTextBeforeCursor`, `getTextAfterCursor`, `getSelectedText`).
  - Transactional undo coalescing: typing and swipes join previous undo steps so backspaces and continuous writing don't pollute the undo stack.
  - Dynamic `WindowInsets` caret visibility: the viewport automatically scrolls up so the caret is never hidden behind the soft keyboard.

- **Android Scoped Storage (SAF) & Formats**:
  - Native document picker (`ACTION_OPEN_DOCUMENT` and `ACTION_CREATE_DOCUMENT`) reading and writing byte streams without raw POSIX path restrictions.
  - Opens and saves `.docx`, `.odt`, `.rtf`, `.md`, and `.txt`.
  - Exports tagged and marked-up `.pdf` files.
  - Native Android Share Sheet via `FileProvider`.

---

## 🏗️ Architecture

```mermaid
flowchart TD
    subgraph JetpackCompose ["Jetpack Compose Material 3 Shell"]
        StateFlow["StateFlow<EditorUiState>"]
        InteractingFlow["StateFlow<Boolean> (isInteracting Toolbar Fade)"]
        TopBar["Top App Bar (Undo/Redo, Title, Share, Open/Save)"]
        FormatBar["Contextual Keyboard Formatting Bar"]
        FloatingBar["Animated Floating Action Mode (Cut, Copy, Paste, Comment)"]
        Handles["Teardrop Selection Handles (Start & End with Crossover Swap)"]
        Magnifier["Floating Magnifier Lens (Touch Baseline Focus)"]
        CanvasHost["WordcraftCanvas (SurfaceView + OverScroller Spline Fling)"]
        CustomIME["WordcraftInputConnection (Swipe, Autocomplete, Batch Undo)"]
    end

    subgraph JNI_Layer ["Split JNI Bridge (libwordcraft_android.so)"]
        CmdJNI["Low-Frequency: Command Dispatcher (dispatchCommand)"]
        HotJNI["High-Frequency Hot Path: Raw Primitives (Touch / Scale / Fling / IME / Handle Drag)"]
        StatePush["Bi-Directional State Push (Rust -> Kotlin StateFlow)"]
    end

    subgraph RustCore ["WordCraft Engine & WGPU Canvas"]
        SurfaceRenderer["AndroidRenderer (Vulkan / OpenGL ES Fallback)"]
        Session["Session (Document, Selection, Undo/Redo)"]
        DocLayout["DocLayout (Pagination, Hit-Testing, Caret Coordinates)"]
        DocRender["wordcraft-render (vello_cpu Rasterizer)"]
        TouchMachine["Kinetic Momentum, Pinch Scale & Selection Crossover Engine"]
    end

    StatePush -->|Push EditorUiState| StateFlow
    StateFlow --> TopBar
    StateFlow --> FormatBar
    StateFlow --> FloatingBar
    StateFlow --> Handles
    InteractingFlow --> FloatingBar

    TopBar -->|Format / Style / Tables| CmdJNI
    FormatBar -->|Bold / Italic / Align| CmdJNI
    FloatingBar -->|Cut / Copy / Paste| CmdJNI
    CmdJNI -->|Session::run(id, params)| Session

    CanvasHost -->|nativeOnScroll / nativeOnScale / nativeOnFling| HotJNI
    CanvasHost -->|nativeOnTap / nativeOnDoubleTap / nativeOnLongPress| HotJNI
    Handles -->|nativeOnDragSelectionHandle| HotJNI
    CustomIME -->|nativeImeCommit / nativeImeSetComposing| HotJNI

    HotJNI --> TouchMachine
    HotJNI --> Session
    Session --> DocLayout
    DocLayout --> StatePush

    CanvasHost -->|ANativeWindow_fromSurface| SurfaceRenderer
    DocLayout --> DocRender
    DocRender --> SurfaceRenderer
```

---

## 🚀 Building & Running

### Requirements
- Android SDK (API 34) & NDK 26+
- JDK 17+
- Rust toolchain (`rustup target add aarch64-linux-android x86_64-linux-android`)
- `cargo-ndk` (`cargo install cargo-ndk`)

### Option 1: Android Studio (Recommended)
1. Open the project root in Android Studio:
   `apps/wordcraft-android/android`
2. Android Studio will automatically sync the Gradle build and download the NDK/SDK components.
3. Run on a connected Android device or emulator (`Shift + F10`).

### Option 2: Command Line
1. Build the Rust native libraries and APK:
   ```bash
   ./apps/wordcraft-android/build_apk.sh --debug
   ```
2. The generated APK will be placed in:
   `apps/wordcraft-android/android/app/build/outputs/apk/debug/app-debug.apk`

---

## 📄 License & Attribution

- WordCraft Android is licensed under **MIT OR Apache-2.0**.
- Built on top of the clean-room, open-source [`storytold/wordcraft`](https://github.com/storytold/wordcraft) engine by the ArtCraft team.
- No proprietary Microsoft, Adobe, Avid, or Autodesk assets or iconography are used. All icons and UI elements are custom Material 3 / code-drawn.

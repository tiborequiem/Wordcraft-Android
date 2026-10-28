#!/usr/bin/env bash
set -euo pipefail

# WordCraft Android Build & APK Packaging Script
# Usage: ./build_apk.sh [--release|--debug]

MODE="${1:---debug}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
JNILIBS_DIR="${SCRIPT_DIR}/android/app/src/main/jniLibs"

echo "=== WordCraft Android Build Pipeline ==="
echo "Workspace: ${WORKSPACE_ROOT}"
echo "Mode:      ${MODE}"
echo "JNI Libs:  ${JNILIBS_DIR}"

# 1. Resolve Android SDK & NDK paths automatically
if [ -z "${ANDROID_HOME:-}" ]; then
    if [ -d "$HOME/Android/Sdk" ]; then
        export ANDROID_HOME="$HOME/Android/Sdk"
        echo "Auto-detected ANDROID_HOME: ${ANDROID_HOME}"
    fi
fi

if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -n "${ANDROID_HOME:-}" ]; then
    if [ -d "${ANDROID_HOME}/ndk" ]; then
        NDK_CANDIDATE=$(find "${ANDROID_HOME}/ndk" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort -V | tail -n 1 || true)
        if [ -n "${NDK_CANDIDATE}" ]; then
            export ANDROID_NDK_HOME="${NDK_CANDIDATE}"
            export NDK_HOME="${NDK_CANDIDATE}"
            echo "Auto-detected ANDROID_NDK_HOME: ${ANDROID_NDK_HOME}"
        fi
    fi
fi

# Ensure cargo-ndk in PATH
export PATH="$HOME/.cargo/bin:$PATH"

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    echo ""
    echo "⚠️  Android NDK not detected in '$HOME/Android/Sdk/ndk'."
    echo "Please open Android Studio -> Tools -> SDK Manager -> SDK Tools tab -> check 'NDK (Side by side)' -> Apply."
    echo ""
fi

# 2. Compile Rust native cdylib
echo "--> Compiling wordcraft-android cdylib..."
CARGO_FLAGS=()
if [ "${MODE}" = "--release" ]; then
    CARGO_FLAGS+=("--release")
fi

mkdir -p "${JNILIBS_DIR}"

if command -v cargo-ndk &> /dev/null && [ -n "${ANDROID_NDK_HOME:-}" ]; then
    (
        cd "${WORKSPACE_ROOT}"
        cargo ndk -t arm64-v8a -t x86_64 -P 26 -o "${JNILIBS_DIR}" build "${CARGO_FLAGS[@]}" -p wordcraft-android
    )
    echo "--> Native libraries built successfully in ${JNILIBS_DIR}"
else
    echo "--> Skipping cargo-ndk build step until NDK is installed."
fi

# 3. Assemble Android APK via Gradle
echo "--> Assembling Android APK..."
cd "${SCRIPT_DIR}/android"

if [ -f "./gradlew" ]; then
    GRADLE_CMD="./gradlew"
elif command -v gradle &> /dev/null; then
    GRADLE_CMD="gradle"
else
    echo ""
    echo "To assemble the APK:"
    echo "1. Open Android Studio."
    echo "2. Open folder: ${SCRIPT_DIR}/android"
    echo "3. Click Build -> Build Bundle(s) / APK(s) -> Build APK(s)."
    echo ""
    exit 0
fi

if [ "${MODE}" = "--release" ]; then
    ${GRADLE_CMD} assembleRelease
else
    ${GRADLE_CMD} assembleDebug
fi

echo "=== Build Complete ==="

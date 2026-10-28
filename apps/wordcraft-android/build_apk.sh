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

# 1. Check Cargo NDK
if ! command -v cargo-ndk &> /dev/null; then
    echo "cargo-ndk not found in PATH."
    echo "Install via: cargo install cargo-ndk"
    echo "Ensure target architectures are installed:"
    echo "  rustup target add aarch64-linux-android x86_64-linux-android"
fi

# 2. Compile Rust native cdylib
echo "--> Compiling wordcraft-android cdylib..."
CARGO_FLAGS=()
if [ "${MODE}" = "--release" ]; then
    CARGO_FLAGS+=("--release")
fi

mkdir -p "${JNILIBS_DIR}"

if command -v cargo-ndk &> /dev/null; then
    (
        cd "${WORKSPACE_ROOT}"
        cargo ndk -t arm64-v8a -t x86_64 -o "${JNILIBS_DIR}" build "${CARGO_FLAGS[@]}" -p wordcraft-android
    )
    echo "--> Native libraries built successfully in ${JNILIBS_DIR}"
else
    echo "--> Skipping cargo-ndk compilation (cargo-ndk not found). Pre-built .so expected in ${JNILIBS_DIR}"
fi

# 3. Assemble Android APK via Gradle
echo "--> Assembling Android APK..."
cd "${SCRIPT_DIR}/android"

if [ -f "./gradlew" ]; then
    GRADLE_CMD="./gradlew"
elif command -v gradle &> /dev/null; then
    GRADLE_CMD="gradle"
else
    echo "Neither ./gradlew nor system gradle found."
    echo "Open the project in Android Studio at:"
    echo "  ${SCRIPT_DIR}/android"
    echo "Android Studio will automatically download the Gradle wrapper and assemble the APK."
    exit 0
fi

if [ "${MODE}" = "--release" ]; then
    ${GRADLE_CMD} assembleRelease
else
    ${GRADLE_CMD} assembleDebug
fi

echo "=== Build Complete ==="

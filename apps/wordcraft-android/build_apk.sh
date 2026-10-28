#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

echo "=== WordCraft Android Build Script ==="

# 1. Detect Android SDK & NDK
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    NDK_LATEST=$(ls -d "$ANDROID_HOME/ndk/"* 2>/dev/null | tail -n 1 || true)
    if [ -n "$NDK_LATEST" ]; then
        export ANDROID_NDK_HOME="$NDK_LATEST"
        export NDK_HOME="$NDK_LATEST"
    fi
fi

echo "Android SDK: $ANDROID_HOME"
echo "Android NDK: ${ANDROID_NDK_HOME:-not found}"

# 2. Detect / configure compatible JDK (prefer Java 17/21)
if [ -d "/tmp/jdk17" ]; then
    export JAVA_HOME="/tmp/jdk17"
    export PATH="$JAVA_HOME/bin:$PATH"
fi
echo "Java Version: $(java -version 2>&1 | head -n 1)"

# 3. Build native shared library via cargo ndk
echo "Building libwordcraft_android.so (arm64-v8a)..."
cargo ndk -t arm64-v8a -P 28 -o "$ROOT_DIR/apps/wordcraft-android/jniLibs" build --release -p wordcraft-android

# 4. Assemble APK via Gradle
echo "Packaging APK with Gradle..."
cd "$ROOT_DIR/apps/wordcraft-android/android"
./gradlew assembleDebug

# 5. Copy output APK
APK_OUT=$(find "$ROOT_DIR/apps/wordcraft-android/android/build/outputs/apk/debug" -name "*.apk" | head -n 1)
if [ -n "$APK_OUT" ]; then
    /bin/cp -f "$APK_OUT" "$ROOT_DIR/wordcraft-android-v0.10.0-debug.apk"
    echo "=== SUCCESS! APK ready at: $ROOT_DIR/wordcraft-android-v0.10.0-debug.apk ==="
fi

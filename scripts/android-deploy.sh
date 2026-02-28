#!/usr/bin/env bash
# Build the Android app and deploy it to a connected device.
#
# Usage:
#   ./scripts/android-deploy.sh          # full rebuild (native + APK + install)
#   ./scripts/android-deploy.sh --apk    # skip native rebuild, just APK + install
#
# Requires: nix develop .#android (provides cargo-ndk, Android SDK, JDK, adb)
set -euo pipefail

project_root="$(git rev-parse --show-toplevel)"
jni_dir="${project_root}/subportal-android/app/src/main/jniLibs"
bindings_dir="${project_root}/subportal-android/app/src/main/java"
apk_path="${project_root}/subportal-android/app/build/outputs/apk/debug/app-debug.apk"

skip_native=false
if [[ "${1:-}" == "--apk" ]]; then
    skip_native=true
fi

if [[ "$skip_native" == false ]]; then
    echo "--- Cross-compiling native libraries ---"
    cargo ndk \
        -t arm64-v8a \
        -t x86_64 \
        -o "$jni_dir" \
        build -p subportal-android-core --release

    echo "--- Regenerating UniFFI Kotlin bindings ---"
    # Build the host .so for bindgen (needs to match the host arch, not Android)
    cargo build -p subportal-android-core
    cargo run -p uniffi-bindgen generate \
        --library "${project_root}/target/debug/libsubportal_android_core.so" \
        --language kotlin \
        --out-dir "$bindings_dir"
fi

echo "--- Building debug APK ---"
"${project_root}/subportal-android/gradlew" \
    -p "${project_root}/subportal-android" \
    assembleDebug

echo "--- Installing on device ---"
adb install -r "$apk_path"

echo "--- Done ---"

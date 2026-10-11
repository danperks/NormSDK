#!/usr/bin/env bash
# Builds NormSDK's native libraries and regenerates the bindings.
#
#   scripts/build.sh apple     # dist/apple/NormSDKFFI.xcframework + Sources/NormSDK
#   scripts/build.sh android   # android/normsdk (jniLibs + Kotlin), needs the NDK
#   scripts/build.sh python    # python/normsdk (host library + bindings)
#   scripts/build.sh all
#
# Generated files are committed so apps can depend on the repo directly.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CRATE=norm-sdk
LIB=libnormsdk
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"

log() { printf '\033[1m==> %s\033[0m\n' "$*"; }

host_lib() {
  cargo build --release -p "$CRATE" --lib >&2
  case "$(uname -s)" in
    Darwin) echo "target/release/$LIB.dylib" ;;
    *) echo "target/release/$LIB.so" ;;
  esac
}

bindgen() { # language out_dir
  cargo run -q --release -p uniffi-bindgen -- generate "$(host_lib)" --language "$1" --out-dir "$2" --no-format
}

build_apple() {
  local targets=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin)
  rustup target add "${targets[@]}" >/dev/null
  for t in "${targets[@]}"; do
    log "cargo build $t"
    cargo build --release -p "$CRATE" --lib --target "$t"
  done

  local work=target/apple
  rm -rf "$work" && mkdir -p "$work"/{ios,sim,macos,gen,headers/NormSDKFFI}
  cp "target/aarch64-apple-ios/release/$LIB.a" "$work/ios/$LIB.a"
  lipo -create "target/aarch64-apple-ios-sim/release/$LIB.a" "target/x86_64-apple-ios/release/$LIB.a" -output "$work/sim/$LIB.a"
  lipo -create "target/aarch64-apple-darwin/release/$LIB.a" "target/x86_64-apple-darwin/release/$LIB.a" -output "$work/macos/$LIB.a"
  for slice in ios sim macos; do strip -S -x "$work/$slice/$LIB.a"; done

  log "generate Swift bindings"
  bindgen swift "$work/gen"
  cp "$work/gen/NormSDKFFI.h" "$work/headers/NormSDKFFI/"
  # Headers live in a subdirectory so this module map can't collide with
  # another static XCFramework's in the app's shared include directory.
  cat > "$work/headers/NormSDKFFI/module.modulemap" <<'EOF'
module NormSDKFFI {
    header "NormSDKFFI.h"
    export *
}
EOF
  mkdir -p Sources/NormSDK
  cp "$work/gen/NormSDK.swift" Sources/NormSDK/NormSDK.swift

  log "create XCFramework"
  rm -rf dist/apple/NormSDKFFI.xcframework
  mkdir -p dist/apple
  xcodebuild -create-xcframework \
    -library "$work/ios/$LIB.a" -headers "$work/headers" \
    -library "$work/sim/$LIB.a" -headers "$work/headers" \
    -library "$work/macos/$LIB.a" -headers "$work/headers" \
    -output dist/apple/NormSDKFFI.xcframework >/dev/null
  du -sh dist/apple/NormSDKFFI.xcframework
}

build_android() {
  if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    for candidate in "${ANDROID_HOME:-$HOME/Library/Android/sdk}"/ndk/*; do ANDROID_NDK_HOME="$candidate"; done
    export ANDROID_NDK_HOME
  fi
  [[ -d "${ANDROID_NDK_HOME:-}" ]] || { echo "Set ANDROID_NDK_HOME to an Android NDK" >&2; exit 1; }
  command -v cargo-ndk >/dev/null || cargo install cargo-ndk
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android >/dev/null

  local jni=android/normsdk/src/main/jniLibs
  rm -rf "$jni"
  log "cargo ndk (arm64-v8a, armeabi-v7a, x86_64)"
  cargo ndk --platform 24 -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$jni" build --release -p "$CRATE" --lib

  log "generate Kotlin bindings"
  local gen=target/kotlin
  rm -rf "$gen"
  bindgen kotlin "$gen"
  rm -rf android/normsdk/src/main/kotlin/io/github/danperks/normsdk/normsdk.kt
  mkdir -p android/normsdk/src/main/kotlin/io/github/danperks/normsdk
  cp "$gen/io/github/danperks/normsdk/normsdk.kt" android/normsdk/src/main/kotlin/io/github/danperks/normsdk/
  du -sh "$jni"/*
}

build_python() {
  log "build host library + Python bindings"
  local lib
  lib="$(host_lib)"
  local gen=target/python
  rm -rf "$gen" && mkdir -p "$gen"
  bindgen python "$gen"
  cp "$gen/normsdk.py" python/normsdk/normsdk.py
  rm -f python/normsdk/"$LIB".*
  cp "$lib" python/normsdk/
}

case "${1:-all}" in
  apple) build_apple ;;
  android) build_android ;;
  python) build_python ;;
  all) build_python; build_apple; build_android ;;
  *) echo "usage: $0 [apple|android|python|all]" >&2; exit 2 ;;
esac

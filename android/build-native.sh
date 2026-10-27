#!/usr/bin/env bash
# Cross-compiles the Rust engine into app/src/main/jniLibs.
# Needs: rustup targets for android, cargo-ndk, and ANDROID_NDK_HOME set.
set -euo pipefail
cd "$(dirname "$0")/../cli"
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android >/dev/null 2>&1 || true
cargo ndk   -t arm64-v8a   -t armeabi-v7a   -t x86_64   -o ../android/app/src/main/jniLibs   build --release --features android

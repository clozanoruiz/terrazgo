#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Lints the app as it is built for Android: the shell and every workspace crate
# it pulls in, compiled for aarch64-linux-android, with clippy's warnings as
# errors — the bar CI holds the desktop build to.
#
# WHY IT EXISTS: CI builds and lints for Linux only, so the code behind
# `#[cfg(mobile)]` (the geolocation plugin, the mobile entry point) is never
# compiled there, and neither is the Android side of anything the standard
# library implements per target. The first run found one of the latter: a
# clippy false positive on a `thread_local!` (crates/terrazgo-core/src/db.rs
# says why). It is a release step (docs/maintenance.md §6) rather than a CI
# job because the job would need the NDK the CI runner deletes to make room,
# plus a second dependency tree's worth of minutes and cache.
#
# NEEDS, on a Linux host: the Rust target (`rustup target add
# aarch64-linux-android`), a built frontend (`npm run build` — tauri embeds
# dist/ at compile time and its error does not say so), and an Android NDK:
# `NDK_HOME` if set, otherwise the newest under `$ANDROID_HOME/ndk` (default
# ~/Android/Sdk/ndk). No Java, Gradle or device — it compiles and lints, it
# builds no APK. The NDK is only there for C code: the bundled SQLite.

set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=aarch64-linux-android
# The API level the C compiler targets. Must match `minSdk` in
# src-tauri/gen/android/app/build.gradle.kts.
API=24

rustup target list --installed | grep -qx "$TARGET" \
  || { echo "missing Rust target: rustup target add $TARGET" >&2; exit 1; }
[ -f dist/index.html ] \
  || { echo "dist/ is missing: run npm run build first" >&2; exit 1; }

if [ -z "${NDK_HOME:-}" ]; then
  NDK_ROOT="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk"
  NDK_HOME="$(ls -d "$NDK_ROOT"/*/ 2>/dev/null | sort -V | tail -1)"
  [ -n "$NDK_HOME" ] || { echo "no Android NDK under $NDK_ROOT; set NDK_HOME" >&2; exit 1; }
fi
BIN="${NDK_HOME%/}/toolchains/llvm/prebuilt/linux-x86_64/bin"
CLANG="$BIN/${TARGET}${API}-clang"
[ -x "$CLANG" ] || { echo "no $(basename "$CLANG") in $BIN" >&2; exit 1; }

echo "linting for $TARGET with NDK $(basename "${NDK_HOME%/}")"
CC_aarch64_linux_android="$CLANG" \
AR_aarch64_linux_android="$BIN/llvm-ar" \
  cargo clippy -p terrazgo --lib --target "$TARGET" -- -D warnings

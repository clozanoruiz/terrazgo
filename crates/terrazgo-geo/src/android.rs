// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android-only TLS bootstrap.
//!
//! `rustls-platform-verifier` delegates certificate verification to the
//! Android trust store through JNI, and it panics on first use if it was
//! never handed the JVM + app context. The first on-device test (2026-07-17)
//! hit exactly that: the panic killed a tokio worker mid-`get_map_style`,
//! the invoke never resolved, and the map stayed silently blank.
//!
//! The init is lazy and lives at the fetch chokepoint (`fetch::http_get`)
//! rather than at app startup, because startup races activity creation: the
//! Rust main thread is spawned from the process-lifecycle `onCreate` while
//! tao captures the activity context in the activity's own `onCreate`. A
//! network fetch, by contrast, can only be triggered once the webview exists
//! — and the webview lives inside the activity — so by then the context is
//! guaranteed. It also keeps the whole TLS trust policy in this crate, next
//! to the `RootCerts::PlatformVerifier` choice it completes.
//!
//! The verifier's Kotlin half (`org.rustls.platformverifier.CertificateVerifier`)
//! must be compiled into the APK: `src-tauri/gen/android/app/build.gradle.kts`
//! pulls it from the crate's bundled Maven repository via `cargo metadata`,
//! so the Kotlin version tracks this crate's version automatically.

use std::sync::OnceLock;

/// Set once the verifier holds its JNI handles; later fetches skip the JNI
/// round-trip entirely.
static VERIFIER_READY: OnceLock<()> = OnceLock::new();

/// Hand `rustls-platform-verifier` the JVM + activity context if it does not
/// have them yet. Idempotent and race-safe (the verifier's own init is
/// `get_or_try_init`). A failure is returned — and retried on the next fetch
/// — instead of cached: the one realistic cause is "activity not created
/// yet", which heals itself.
pub(crate) fn ensure_platform_verifier() -> Result<(), String> {
    if VERIFIER_READY.get().is_some() {
        return Ok(());
    }
    let ctx = tao::platform::android::prelude::main_android_context()
        .ok_or_else(|| "Android activity context not available yet".to_string())?;
    // SAFETY: both raw pointers come from tao's ndk_glue, which captured them
    // at activity creation — the JavaVM pointer is process-global and the
    // context is a JNI global ref tao keeps alive with the activity.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.java_vm.cast()) }
        .map_err(|e| format!("JavaVM handle: {e}"))?;
    let mut env = vm
        .attach_current_thread_as_daemon()
        .map_err(|e| format!("JNI attach: {e}"))?;
    // SAFETY: see above — tao's global ref outlives this borrow, and JObject
    // does not delete the reference on drop.
    let context = unsafe { jni::objects::JObject::from_raw(ctx.context_jobject.cast()) };
    rustls_platform_verifier::android::init_with_env(&mut env, context)
        .map_err(|e| format!("platform verifier init: {e}"))?;
    let _ = VERIFIER_READY.set(());
    Ok(())
}

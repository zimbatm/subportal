# Subportal

## Project structure

- `crates/` -- Rust workspace crates
  - `subportal-lib` -- shared protocol types, wire format
  - `subportal-iroh` -- iroh transport, peer registries, ticket handling
  - `subportal-android-core` -- UniFFI bridge crate (`cdylib`) exposing `SubportalCore` to Kotlin
  - `uniffi-bindgen` -- local binary for regenerating Kotlin bindings
- `subportal-android/` -- Android app (Gradle + Kotlin + Jetpack Compose)
  - UniFFI-generated bindings live at `app/src/main/java/uniffi/subportal_android_core/subportal_android_core.kt`

## Building

### Rust

```sh
nix develop --command cargo test --workspace
nix develop --command cargo build -p subportal-android-core
```

### Android -- full build and deploy

Use the deploy script inside the `android` dev shell:

```sh
nix develop .#android --command ./scripts/android-deploy.sh
```

This does: cross-compile native `.so` via `cargo ndk` -> regenerate UniFFI
Kotlin bindings -> `assembleDebug` -> `adb install`.

To skip the native rebuild (Kotlin-only changes):

```sh
nix develop .#android --command ./scripts/android-deploy.sh --apk
```

### Regenerating UniFFI Kotlin bindings (manual)

After changing Rust types exposed via `#[uniffi::export]` (e.g. `ServerInfo`),
you must rebuild the host `.so` and regenerate:

```sh
cargo build -p subportal-android-core
cargo run -p uniffi-bindgen generate \
  --library target/debug/libsubportal_android_core.so \
  --language kotlin \
  --out-dir subportal-android/app/src/main/java
```

**Important:** You must also cross-compile the Android `.so` with `cargo ndk`,
otherwise the APK will bundle stale native code and crash at runtime with
`BufferUnderflowException`. The deploy script handles both steps.

### Android -- Gradle only

```sh
nix develop .#android --command ./subportal-android/gradlew -p subportal-android assembleDebug
```

## Dev shells

- `default` -- Rust toolchain (cargo, rustc, etc.)
- `android` -- Android SDK, JDK, `cargo-ndk`, adb (needed for native cross-compilation and Gradle builds)

## Architecture notes

- `SubportalService` (foreground service) owns the Rust `SubportalCore` singleton.
- `SubportalCallbackImpl` bridges Rust callbacks to Android (URI open, file open, notifications, connection changes).
- `EventLog` (singleton) stores in-memory per-server event history, keyed by server name.
- Navigation uses Jetpack Compose Navigation (`NavGraph.kt`), routes defined in `Routes` object.
- Rust `ServerEntry.enrolled_at` is exposed as ISO 8601 string via `ServerInfo.enrolledAt`.

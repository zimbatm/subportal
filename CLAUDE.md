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

### Regenerating UniFFI Kotlin bindings

After changing the Rust `ServerInfo` record or any `#[uniffi::export]` types, rebuild the `.so` then regenerate:

```sh
nix develop --command cargo build -p subportal-android-core
nix develop --command cargo run -p uniffi-bindgen generate \
  --library target/debug/libsubportal_android_core.so \
  --language kotlin \
  --out-dir subportal-android/app/src/main/java
```

### Android

```sh
nix develop .#android --command ./subportal-android/gradlew -p subportal-android assembleDebug
```

## Dev shells

- `default` -- Rust toolchain (cargo, rustc, etc.)
- `android` -- Android SDK + Java (needed for Gradle builds)

## Architecture notes

- `SubportalService` (foreground service) owns the Rust `SubportalCore` singleton.
- `SubportalCallbackImpl` bridges Rust callbacks to Android (URI open, file open, notifications, connection changes).
- `EventLog` (singleton) stores in-memory per-server event history, keyed by server name.
- Navigation uses Jetpack Compose Navigation (`NavGraph.kt`), routes defined in `Routes` object.
- Rust `ServerEntry.enrolled_at` is exposed as ISO 8601 string via `ServerInfo.enrolledAt`.

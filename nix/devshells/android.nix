{
  pkgs,
  inputs,
  system,
  ...
}:
let
  fenixPkgs = inputs.fenix.packages.${system};

  # Rust toolchain with Android cross-compilation targets
  rustToolchain = fenixPkgs.combine [
    fenixPkgs.stable.cargo
    fenixPkgs.stable.rustc
    fenixPkgs.stable.rustfmt
    fenixPkgs.stable.clippy
    fenixPkgs.targets.aarch64-linux-android.stable.rust-std
    fenixPkgs.targets.x86_64-linux-android.stable.rust-std
  ];

  androidComposition = pkgs.androidenv.composeAndroidPackages {
    platformVersions = [ "35" ];
    buildToolsVersions = [
      "34.0.0"
      "35.0.0"
    ];
    includeNDK = true;
    ndkVersion = "27.2.12479018";
  };

  androidSdk = androidComposition.androidsdk;
  androidSdkRoot = "${androidSdk}/libexec/android-sdk";
in
pkgs.mkShell {
  packages = [
    rustToolchain
    pkgs.cargo-ndk
    pkgs.pkg-config
    pkgs.jdk17
    pkgs.gradle
    androidSdk
  ];

  buildInputs = [
    pkgs.dbus
  ];

  env = {
    ANDROID_HOME = androidSdkRoot;
    ANDROID_SDK_ROOT = androidSdkRoot;
    ANDROID_NDK_HOME = "${androidSdkRoot}/ndk-bundle";
    ANDROID_NDK_ROOT = "${androidSdkRoot}/ndk-bundle";
    RUST_BACKTRACE = "1";
    GRADLE_OPTS = "-Dorg.gradle.project.android.aapt2FromMavenOverride=${androidSdkRoot}/build-tools/35.0.0/aapt2";
  };
}

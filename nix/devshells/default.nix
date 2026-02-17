{ pkgs }:
pkgs.mkShell {
  packages = [
    pkgs.cargo
    pkgs.rustc
    pkgs.rustfmt
    pkgs.clippy
    pkgs.pkg-config
  ];

  buildInputs = [
    pkgs.dbus
  ];

  env = {
    RUST_BACKTRACE = "1";
  };
}

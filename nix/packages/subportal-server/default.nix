{
  pkgs,
  flake,
  pname,
  ...
}:
pkgs.rustPlatform.buildRustPackage {
  inherit pname;
  version = "0.1.0";

  src = pkgs.lib.sourceByRegex flake [
    "Cargo\.toml"
    "Cargo\.lock"
    "crates"
    "crates/.*"
  ];

  cargoLock.lockFile = "${flake}/Cargo.lock";

  cargoBuildFlags = [
    "-p"
    "subportal"
    "-p"
    "xdg-open"
    "-p"
    "notify-send"
  ];

  cargoTestFlags = [
    "-p"
    "subportal"
    "-p"
    "xdg-open"
    "-p"
    "notify-send"
  ];

  nativeBuildInputs = [ pkgs.scdoc ];

  postInstall = ''
    mkdir -p $out/share/man/man1
    scdoc < crates/subportal/subportal.1.scd > $out/share/man/man1/subportal.1
    scdoc < crates/xdg-open/xdg-open.1.scd > $out/share/man/man1/xdg-open.1
    scdoc < crates/notify-send/notify-send.1.scd > $out/share/man/man1/notify-send.1
  '';

  meta = {
    description = "subportal server-side CLI tools (subportal, xdg-open, notify-send)";
    homepage = "https://git.ntd.one/zimbatm/subportal";
    mainProgram = "subportal";
  };
}

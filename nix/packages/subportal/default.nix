{
  pkgs,
  flake,
  pname,
  ...
}:
pkgs.rustPlatform.buildRustPackage {
  inherit pname;
  version = "0.1.0";

  src = flake;

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

  meta = {
    description = "subportal server-side CLI tools (subportal, xdg-open, notify-send)";
    homepage = "https://git.ntd.one/zimbatm/subportal";
    mainProgram = "subportal";
  };
}

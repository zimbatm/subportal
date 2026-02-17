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
    "subportald"
  ];

  cargoTestFlags = [
    "-p"
    "subportald"
  ];

  nativeBuildInputs = [ pkgs.pkg-config ];

  buildInputs = [ pkgs.dbus ];

  meta = {
    description = "subportal client daemon - forwards desktop requests from remote servers";
    homepage = "https://git.ntd.one/zimbatm/subportal";
    mainProgram = "subportald";
  };
}

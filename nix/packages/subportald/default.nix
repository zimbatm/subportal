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

  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.scdoc
  ];

  buildInputs = [ pkgs.dbus ];

  postInstall = ''
    install -Dm644 crates/subportald/subportald.desktop $out/share/applications/subportald.desktop
    mkdir -p $out/share/man/man1
    scdoc < crates/subportald/subportald.1.scd > $out/share/man/man1/subportald.1
  '';

  meta = {
    description = "subportal client daemon - forwards desktop requests from remote servers";
    homepage = "https://git.ntd.one/zimbatm/subportal";
    mainProgram = "subportald";
  };
}

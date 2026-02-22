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
    "subportal-desktop"
  ];

  cargoTestFlags = [
    "-p"
    "subportal-desktop"
  ];

  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.scdoc
  ];

  buildInputs = [ pkgs.dbus ];

  postInstall = ''
    install -Dm644 crates/subportal-desktop/subportal-desktop.desktop $out/share/applications/subportal-desktop.desktop
    mkdir -p $out/share/man/man1
    scdoc < crates/subportal-desktop/subportal-desktop.1.scd > $out/share/man/man1/subportal-desktop.1
  '';

  meta = {
    description = "subportal client daemon - forwards desktop requests from remote servers";
    homepage = "https://git.ntd.one/zimbatm/subportal";
    mainProgram = "subportal-desktop";
  };
}

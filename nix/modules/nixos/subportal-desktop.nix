{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.subportal-desktop;
in
{
  options.services.subportal-desktop = {
    enable = lib.mkEnableOption "subportal-desktop, the subportal client daemon";

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportal-desktop" { };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # Run as a systemd user service so it has access to the D-Bus session bus
    # and xdg-desktop-portal.
    systemd.user.services.subportal-desktop = {
      description = "subportal client daemon";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}

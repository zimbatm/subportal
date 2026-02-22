{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.subportald;
in
{
  options.services.subportald = {
    enable = lib.mkEnableOption "subportald, the subportal client daemon";

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportald" { };
  };

  config = lib.mkIf cfg.enable {
    # Run as a systemd user service so it has access to the D-Bus session bus
    # and xdg-desktop-portal.
    systemd.user.services.subportald = {
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

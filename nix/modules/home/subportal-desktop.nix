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
    home.packages = [ cfg.package ];

    systemd.user.services.subportal-desktop = {
      Unit = {
        Description = "subportal client daemon";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };

      Service = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;
      };

      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };
  };
}

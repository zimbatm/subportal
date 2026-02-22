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

    user = lib.mkOption {
      type = lib.types.str;
      description = "User account to run subportal-desktop as.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.services.subportal-desktop = {
      enable = true;
      description = "subportal client daemon";
      wantedBy = [ "system-manager.target" ];

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 5;
        User = cfg.user;
      };
    };
  };
}

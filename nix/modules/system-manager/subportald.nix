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

    user = lib.mkOption {
      type = lib.types.str;
      description = "User account to run subportald as.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.subportald = {
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

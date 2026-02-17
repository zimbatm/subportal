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

    port = lib.mkOption {
      type = lib.types.port;
      default = 19494;
      description = "TCP port for subportald to listen on.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.subportald = {
      Unit = {
        Description = "subportal client daemon";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };

      Service = {
        ExecStart = "${lib.getExe cfg.package} --port ${toString cfg.port}";
        Restart = "on-failure";
        RestartSec = 5;
      };

      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };
  };
}

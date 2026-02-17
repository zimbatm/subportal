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

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to open the firewall for subportald.
        Usually not needed since connections come through SSH reverse tunnels.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # Run as a systemd user service so it has access to the D-Bus session bus
    # and xdg-desktop-portal.
    systemd.user.services.subportald = {
      description = "subportal client daemon";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --port ${toString cfg.port}";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
  };
}

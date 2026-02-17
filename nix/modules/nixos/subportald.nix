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

    sshHosts = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [
        "myserver"
        "*.example.com"
      ];
      description = ''
        SSH host patterns to configure RemoteForward for subportal.
        Each entry generates a Host block in the system SSH config with
        a RemoteForward directive for the subportal port.
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
      partOf = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --port ${toString cfg.port}";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    programs.ssh.extraConfig = lib.mkIf (cfg.sshHosts != [ ]) (
      lib.concatMapStrings (host: ''
        Host ${host}
            RemoteForward 127.0.0.1:${toString cfg.port} 127.0.0.1:${toString cfg.port}
      '') cfg.sshHosts
    );
  };
}

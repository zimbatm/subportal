{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.subportald;
  # systemd specifier for ExecStart (%t = XDG_RUNTIME_DIR)
  systemdSocketPath = "%t/subportal/subportal.sock";
  # SSH token for RemoteForward (%i = local UID)
  sshSocketPath = "/run/user/%i/subportal/subportal.sock";
in
{
  options.services.subportald = {
    enable = lib.mkEnableOption "subportald, the subportal client daemon";

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportald" { };

    socketPath = lib.mkOption {
      type = lib.types.str;
      default = systemdSocketPath;
      description = ''
        Unix socket path for subportald to listen on.
        The default uses systemd's %t (XDG_RUNTIME_DIR) specifier.
      '';
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
        a RemoteForward directive for the subportal Unix socket.
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
        ExecStart = "${lib.getExe cfg.package} --socket ${cfg.socketPath}";
        RuntimeDirectory = "subportal";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    programs.ssh.extraConfig = lib.mkIf (cfg.sshHosts != [ ]) (
      lib.concatMapStrings (host: ''
        Host ${host}
            RemoteForward ${sshSocketPath} ${sshSocketPath}
      '') cfg.sshHosts
    );
  };
}

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
  systemdSocketPath = "%t/subportal.sock";
  # SSH token for the local socket path (%i = local UID)
  localSshSocketPath = "/run/user/%i/subportal.sock";
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
      type = lib.types.attrsOf (
        lib.types.submodule {
          options.remoteUid = lib.mkOption {
            type = lib.types.nullOr lib.types.int;
            default = null;
            description = ''
              UID of the remote user. Used to construct the remote socket path
              (/run/user/<uid>/subportal.sock). When null, uses SSH's %i token
              which expands to the local UID.
            '';
          };
        }
      );
      default = { };
      example = {
        "myserver" = { };
        "other-server" = {
          remoteUid = 1001;
        };
      };
      description = ''
        SSH host patterns to configure RemoteForward for subportal.
        Each key is a Host pattern and the value can optionally specify
        a remoteUid if it differs from the local UID.
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
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    programs.ssh.extraConfig = lib.mkIf (cfg.sshHosts != { }) (
      lib.concatStrings (
        lib.mapAttrsToList (
          host: hostCfg:
          let
            remotePath =
              if hostCfg.remoteUid != null then
                "/run/user/${toString hostCfg.remoteUid}/subportal.sock"
              else
                localSshSocketPath;
          in
          ''
            Host ${host}
                RemoteForward ${remotePath} ${localSshSocketPath}
          ''
        ) cfg.sshHosts
      )
    );
  };
}

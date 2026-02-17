{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.subportald;
  defaultSocketPath = "%t/subportal/subportal.sock";
in
{
  options.services.subportald = {
    enable = lib.mkEnableOption "subportald, the subportal client daemon";

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportald" { };

    socketPath = lib.mkOption {
      type = lib.types.str;
      default = defaultSocketPath;
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
        Each entry generates a Host block in the SSH config with
        a RemoteForward directive for the subportal Unix socket.
      '';
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
        ExecStart = "${lib.getExe cfg.package} --socket ${cfg.socketPath}";
        RuntimeDirectory = "subportal";
        Restart = "on-failure";
        RestartSec = 5;
      };

      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };

    programs.ssh.extraConfig = lib.mkIf (cfg.sshHosts != [ ]) (
      lib.concatMapStrings (host: ''
        Host ${host}
            RemoteForward /run/user/%i/subportal/subportal.sock ${cfg.socketPath}
      '') cfg.sshHosts
    );
  };
}

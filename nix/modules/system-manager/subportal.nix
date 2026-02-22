{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.subportal;
in
{
  options.programs.subportal = {
    enable = lib.mkEnableOption "subportal server-side CLI tools";

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportal-server" { };

    xdg-open = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to install the xdg-open drop-in replacement.";
    };

    notify-send = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to install the notify-send drop-in replacement.";
    };

    agent = {
      enable = lib.mkEnableOption "subportal agent systemd service";

      user = lib.mkOption {
        type = lib.types.str;
        description = "User account to run the subportal agent as.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages =
      let
        pkg = cfg.package;
        wrapped = pkgs.symlinkJoin {
          name = "subportal-tools";
          paths = [ pkg ];
          postBuild =
            lib.optionalString (!cfg.xdg-open) ''
              rm -f $out/bin/xdg-open
            ''
            + lib.optionalString (!cfg.notify-send) ''
              rm -f $out/bin/notify-send
            '';
        };
      in
      [ wrapped ];

    systemd.services.subportal-agent = lib.mkIf cfg.agent.enable {
      enable = true;
      description = "subportal agent";
      wantedBy = [ "system-manager.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/subportal agent";
        Restart = "on-failure";
        RestartSec = 5;
        User = cfg.agent.user;
      };
    };
  };
}

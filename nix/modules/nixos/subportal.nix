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

    package = lib.mkPackageOption flake.packages.${pkgs.system} "subportal" { };

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

    agent.enable = lib.mkEnableOption "subportal-agent systemd user service";
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages =
      let
        pkg = cfg.package;
        # Wrap the package to only include selected binaries
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

    # subportal-agent systemd user service
    systemd.user.services.subportal-agent = lib.mkIf cfg.agent.enable {
      description = "subportal agent";
      wantedBy = [ "default.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/subportal-agent";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}

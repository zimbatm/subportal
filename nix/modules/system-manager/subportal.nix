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

    package =
      lib.mkPackageOption flake.packages.${pkgs.stdenv.hostPlatform.system} "subportal-server"
        { };

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

    relayUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Custom relay server URL (e.g. https://relay.example.com). Uses default iroh relays if null.";
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
        # Enable lingering so /run/user/<uid> persists without active
        # login sessions (e.g. after SSH disconnect).
        ExecStartPre = "+${pkgs.systemd}/bin/loginctl enable-linger ${cfg.agent.user}";
        ExecStart = "${cfg.package}/bin/subportal agent${
          lib.optionalString (cfg.relayUrl != null) " --relay-url ${cfg.relayUrl}"
        }";
        Restart = "on-failure";
        RestartSec = 5;
        User = cfg.agent.user;
      };
    };
  };
}

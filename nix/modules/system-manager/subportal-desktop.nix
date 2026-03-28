{ flake, ... }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.subportal-desktop;
in
{
  options.services.subportal-desktop = {
    enable = lib.mkEnableOption "subportal-desktop, the subportal client daemon";

    package =
      lib.mkPackageOption flake.packages.${pkgs.stdenv.hostPlatform.system} "subportal-desktop"
        { };

    relayUrl = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Custom relay server URL (e.g. https://relay.example.com). Uses default iroh relays if null.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      description = "User account to run subportal-desktop as.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.services.subportal-desktop = {
      enable = true;
      description = "subportal client daemon";
      wantedBy = [ "system-manager.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package}${
          lib.optionalString (cfg.relayUrl != null) " run --relay-url ${cfg.relayUrl}"
        }";
        Restart = "on-failure";
        RestartSec = 5;
        User = cfg.user;
      };
    };
  };
}

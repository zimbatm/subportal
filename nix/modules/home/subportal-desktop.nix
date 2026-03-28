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
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    systemd.user.services.subportal-desktop = {
      Unit = {
        Description = "subportal client daemon";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };

      Service = {
        ExecStart = "${lib.getExe cfg.package}${
          lib.optionalString (cfg.relayUrl != null) " run --relay-url ${cfg.relayUrl}"
        }";
        Restart = "on-failure";
        RestartSec = 5;
      };

      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };
  };
}

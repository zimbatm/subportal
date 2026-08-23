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

    agent.enable = lib.mkEnableOption "subportal agent systemd user service";
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
        ExecStart = "${cfg.package}/bin/subportal agent${
          lib.optionalString (cfg.relayUrl != null) " --relay-url ${cfg.relayUrl}"
        }";
        Restart = "on-failure";
        RestartSec = 5;

        # Sandbox by default; every value is a mkDefault so a consumer can
        # loosen what its deployment needs. No ProtectHome: file transfer
        # reads the user's home by design. AF_NETLINK stays: iroh's netmon
        # watches interface changes and crashes without it ("Address family
        # not supported by protocol").
        NoNewPrivileges = lib.mkDefault true;
        LockPersonality = lib.mkDefault true;
        PrivateDevices = lib.mkDefault true;
        PrivateTmp = lib.mkDefault true;
        ProtectClock = lib.mkDefault true;
        ProtectControlGroups = lib.mkDefault true;
        ProtectKernelLogs = lib.mkDefault true;
        ProtectKernelModules = lib.mkDefault true;
        ProtectKernelTunables = lib.mkDefault true;
        ProtectSystem = lib.mkDefault "strict";
        RestrictAddressFamilies = lib.mkDefault [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
          "AF_NETLINK"
        ];
        RestrictNamespaces = lib.mkDefault true;
        RestrictRealtime = lib.mkDefault true;
        RestrictSUIDSGID = lib.mkDefault true;
        SystemCallArchitectures = lib.mkDefault "native";
      };
    };
  };
}

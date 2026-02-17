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
  };

  config = lib.mkIf cfg.enable {
    home.packages =
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
  };
}

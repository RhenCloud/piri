{
  config,
  lib,
  pkgs,
  ...
}:
with lib;
let
  cfg = config.programs.piri;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.programs.piri = {
    enable = mkEnableOption "piri, a daemon for managing niri compositor";

    package = mkOption {
      type = types.package;
      description = "The piri package to use.";
    };

    settings = mkOption {
      type = tomlFormat.type;
      default = { };
      description = "Configuration for piri, see config.example.toml for reference.";
    };

    enableBashIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "Whether to enable Bash integration (completions).";
    };

    enableZshIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "Whether to enable Zsh integration (completions).";
    };

    enableFishIntegration = mkOption {
      type = types.bool;
      default = true;
      description = "Whether to enable Fish integration (completions).";
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."niri/piri.toml" = mkIf (cfg.settings != { }) {
      source = tomlFormat.generate "piri.toml" cfg.settings;
    };

    systemd.user.services.piri = {
      Unit = {
        Description = "piri daemon";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/piri daemon";
        Restart = "on-failure";
      };
      Install = {
        WantedBy = [ "graphical-session.target" ];
      };
    };

    programs.bash.initExtra = mkIf cfg.enableBashIntegration ''
      source <(${cfg.package}/bin/piri completion bash)
    '';

    programs.zsh.initExtra = mkIf cfg.enableZshIntegration ''
      source <(${cfg.package}/bin/piri completion zsh)
    '';

    programs.fish.shellInit = mkIf cfg.enableFishIntegration ''
      ${cfg.package}/bin/piri completion fish | source
    '';
  };
}

{
  config,
  lib,
  pkgs,
  ...
}:
with lib;
let
  cfg = config.programs.piri;
in
{
  options.programs.piri = {
    enable = mkEnableOption "piri, a daemon for managing niri compositor";

    package = mkOption {
      type = types.package;
      description = "The piri package to use.";
    };

    settings = mkOption {
      type = types.attrsOf types.anything;
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
    environment.systemPackages = [ cfg.package ];

    systemd.user.services.piri = {
      description = "piri daemon";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/piri --config ${pkgs.writeText "piri.toml" (builtins.toJSON cfg.settings)} daemon";
        Restart = "on-failure";
      };
    };

    programs.bash.interactiveShellInit = mkIf cfg.enableBashIntegration ''
      source <(${cfg.package}/bin/piri completion bash)
    '';

    programs.zsh.interactiveShellInit = mkIf cfg.enableZshIntegration ''
      source <(${cfg.package}/bin/piri completion zsh)
    '';

    programs.fish.shellInit = mkIf cfg.enableFishIntegration ''
      ${cfg.package}/bin/piri completion fish | source
    '';
  };
}

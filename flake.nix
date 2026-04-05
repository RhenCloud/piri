{
  description = "Piri";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.rust-bin.beta.latest.default;
          rustc = pkgs.rust-bin.beta.latest.default;
        };
        piri = rustPlatform.buildRustPackage {
          pname = "piri";
          version = "0.1.5";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };
      in
      {
        packages.default = piri;

        nixosModules.default =
          { pkgs, ... }:
          {
            imports = [ ./nix/module.nix ];
            config.programs.piri.package = pkgs.lib.mkDefault piri;
          };
        homeManagerModules.default =
          { pkgs, ... }:
          {
            imports = [ ./nix/home-manager.nix ];
            config.programs.piri.package = pkgs.lib.mkDefault piri;
          };
      }
    );
}

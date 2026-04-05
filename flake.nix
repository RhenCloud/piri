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
      in
      {
        packages.default = rustPlatform.buildRustPackage {
          pname = "piri";
          version = "0.1.5";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };

        nixosModules.default = import ./nix/module.nix;
      }
    );
}

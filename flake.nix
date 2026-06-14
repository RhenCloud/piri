{
  description = "Piri";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    let
      outputs = flake-utils.lib.eachDefaultSystem (
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
            buildInputs = with pkgs; [
              wayland
              cairo
              glib
            ];
          };
        in
        {
          packages.default = piri;
          devShells.default = pkgs.mkShell {
            inputsFrom = [ piri ];
            packages = with pkgs; [
              rustfmt
              clippy
              cargo-watch
              rust-analyzer
            ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );
    in
    outputs
    // {
      nixosModules.default =
        { pkgs, ... }:
        {
          imports = [ ./nix/module.nix ];
          config.programs.piri.package = nixpkgs.lib.mkDefault self.packages.${pkgs.system}.default;
        };
      homeManagerModules.default =
        { pkgs, ... }:
        {
          imports = [ ./nix/home-manager.nix ];
          config.programs.piri.package = nixpkgs.lib.mkDefault self.packages.${pkgs.system}.default;
        };
    };
}

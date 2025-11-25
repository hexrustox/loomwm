{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=ee09932cedcef15aaf476f9343d1dea2cb77e261";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;

        mkShell = pkgs.mkShell.override {
          stdenv = pkgs.stdenvNoCC.override {
            cc = null;
            preHook = "";
            allowedRequisites = null;
            initialPath = [ pkgs.coreutils ];
            shell = lib.getExe pkgs.bash;
            extraNativeBuildInputs = [ ];
          };
        };
        override = import ./override/flake.nix pkgs;
      in
      {
        devShells.default = mkShell (
          {
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          }
          // override
          // {
            buildInputs =
              with pkgs;
              [
                gcc
                rustc
                cargo
              ]
              ++ override.buildInputs or [ ];
            packages =
              with pkgs;
              [
                rustfmt
                clippy
                rust-analyzer
                cargo-deny
                cargo-edit
                cargo-machete
                codebook
              ]
              ++ override.packages or [ ];
          }
        );
      }
    );
}

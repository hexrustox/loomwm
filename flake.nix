{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=3cbe716e2346710d6e1f7c559363d14e11c32a43";
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
        mkShell = import ./nix/shell.nix pkgs;
        override = import ./nix/override.nix pkgs;
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
                cargo-deny
                cargo-edit
                rustfmt
                clippy
                rust-analyzer
                codebook
              ]
              ++ override.buildInputs;
          }
        );
      }
    );
}

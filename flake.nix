{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    capsule.url = "gitlab:codnixus/capsule";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      capsule,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        host-pkgs = import <nixpkgs> { };
        capsule-lib = capsule.lib {
          inherit pkgs;
          name = "loomwm";
          devTools =
            with host-pkgs;
            [
              nil
              nixfmt
            ]
            ++ (with pkgs; [
              rust-analyzer
              {
                pkg = cargo;
                extraOpts = [
                  "-t"
                  "--workdir=$(pwd)"
                ];
              }
              {
                pkg = codebook;
                name = "codebook-lsp";
              }
            ]);
          runtimeDeps =
            with host-pkgs;
            [ nix ]
            ++ (with pkgs; [
              gcc
              rustc
              rustfmt
              clippy
              cargo-deny
              cargo-edit
              cargo-machete

              pkg-config
              alacritty
            ]);
          extraOpts = [
            "--pid host"
            "--uts host"

            "--env=CARGO_HOME"
            "--env=COLORTERM=truecolor"
            "--env=GBM_BACKENDS_PATH"
            "--env=HOME"
            "--env=LD_LIBRARY_PATH"
            "--env=LIBGL_DRIVERS_PATH"
            "--env=LIBRARY_PATH"
            "--env=LIBVA_DRIVERS_PATH"
            "--env=PKG_CONFIG_PATH"
            "--env=RUST_SRC_PATH"
            "--env=TERM=xterm-256color"
            "--env=__EGL_VENDOR_LIBRARY_FILENAMES"

            "--tmpfs=/tmp"
            "--tmpfs=\"$HOME\""

            "--volume=/etc/fonts:/etc/fonts:ro"
            "--volume=\"$HOME/.cache\":\"$HOME/.cache\""
            "--volume=\"$XDG_DATA_HOME/cargo\":\"$HOME/.cargo\""

            "--env=WAYLAND_DISPLAY"
            "--env=XDG_RUNTIME_DIR=/tmp/runtime"
            "--volume=$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/runtime/$WAYLAND_DISPLAY:ro"

            "--device=/dev/dri"
          ];
          image = "alpine:latest";
        };
      in
      {
        devShells.default =
          let
            lib = pkgs.lib;
            mesa = pkgs.mesa;
            mesa-drivers = [ mesa ];
            vadrivers = [ ];
            libvdpau = [ pkgs.libvdpau-va-gl ];
            ld = with pkgs; [
              libglvnd
              wayland
            ];
          in
          pkgs.mkShellNoCC {
            inherit (capsule-lib) packages;
            shellHook = ''
              export XDG_DATA_HOME=''${XDG_DATA_HOME:-~/.local/share}
              export CARGO_HOME="$XDG_DATA_HOME/cargo"

              mkdir -p "$CARGO_HOME"

              ${capsule-lib.shellHook}
            '';

            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

            LIBRARY_PATH = "${pkgs.lib.makeLibraryPath [ pkgs.libxkbcommon ]}";
            PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" [ pkgs.libxkbcommon ]}";

            GBM_BACKENDS_PATH = "${lib.makeSearchPathOutput "lib" "lib/gbm" mesa-drivers}";
            LIBGL_DRIVERS_PATH = "${lib.makeSearchPathOutput "lib" "lib/dri" mesa-drivers}";
            LIBVA_DRIVERS_PATH = "${lib.makeSearchPathOutput "out" "lib/dri" (mesa-drivers ++ vadrivers)}";
            __EGL_VENDOR_LIBRARY_FILENAMES = "${mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
            LD_LIBRARY_PATH = "${lib.makeLibraryPath (mesa-drivers ++ ld)}:${
              lib.makeSearchPathOutput "lib" "lib/vdpau" libvdpau
            }";
          };
      }
    );
}

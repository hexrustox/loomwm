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
            ]);
          extraOpts = [
            "--pid host"
            "--uts host"
            "--env=HOME"
            "--env=TERM=xterm-256color"
            "--env=COLORTERM=truecolor"
            "--tmpfs=\"$HOME\""
            "--tmpfs=/tmp"
            "--volume=\"$XDG_DATA_HOME/cargo\":\"$HOME/.cargo\""
            "--volume=\"$HOME/.cache\":\"$HOME/.cache\""

            "--env=XDG_RUNTIME_DIR=/tmp/runtime"
            "--env=WAYLAND_DISPLAY"
            "--volume=$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/runtime/$WAYLAND_DISPLAY:ro"
            "--device=/dev/dri"

            "--env=RUST_SRC_PATH"
            "--env=LIBRARY_PATH=\"${pkgs.lib.makeLibraryPath [ pkgs.libxkbcommon ]}\""
            "--env=PKG_CONFIG_PATH=\"${pkgs.lib.makeSearchPath "lib/pkgconfig" [ pkgs.libxkbcommon ]}\""
            "--env=GBM_BACKENDS_PATH"
            "--env=LIBGL_DRIVERS_PATH"
            "--env=LIBVA_DRIVERS_PATH"
            "--env=__EGL_VENDOR_LIBRARY_FILENAMES"
            "--env=LD_LIBRARY_PATH"

            "-v /etc/static/ssl/certs:/etc/ssl/certs:ro"
          ];
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

              mkdir -p "$XDG_DATA_HOME/cargo"

              ${capsule-lib.shellHook}
            '';

            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
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

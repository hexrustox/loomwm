{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-25.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    capsule.url = "gitlab:codnixus/capsule";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      capsule,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { overlays = [ rust-overlay.overlays.default ]; };
      in
      {
        devShells.default =
          let
            lib = pkgs.lib;
            mesa = pkgs.mesa;
            mesa-drivers = [ mesa ];
            ld = with pkgs; [
              libglvnd
              wayland
              libxkbcommon
            ];
          in
          pkgs.mkShellNoCC {
            packages = with pkgs; [
              clang
              mold
              (rust-bin.stable."1.93.1".default.override {
                extensions = [
                  "rust-src" # for rust-analyzer
                  "rust-analyzer"
                ];
              })
              pkg-config

              alacritty
              firefox
              fastfetch
            ];

            LIBRARY_PATH = "${pkgs.lib.makeLibraryPath [ pkgs.libxkbcommon ]}";
            PKG_CONFIG_PATH = "${pkgs.lib.makeSearchPath "lib/pkgconfig" [ pkgs.libxkbcommon ]}";
            LD_LIBRARY_PATH = "${lib.makeLibraryPath (mesa-drivers ++ ld ++ [ pkgs.vulkan-loader ])}";
            GBM_BACKENDS_PATH = "${lib.makeSearchPathOutput "lib" "lib/gbm" mesa-drivers}";
            LIBGL_DRIVERS_PATH = "${lib.makeSearchPathOutput "lib" "lib/dri" mesa-drivers}";
            __EGL_VENDOR_LIBRARY_FILENAMES = "${mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
            VK_ICD_FILENAMES = "${mesa}/share/vulkan/icd.d";
          };
      }
    );
}

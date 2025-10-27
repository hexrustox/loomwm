pkgs: {
  buildInputs = with pkgs; [
    pkg-config
    libxkbcommon
  ];
  shellHook =
    let
      lib = pkgs.lib;
      mesa = pkgs.mesa;
      mesa-drivers = [ mesa ];
      vadrivers = [ ];
      libvdpau = [ pkgs.libvdpau-va-gl ];
      ld = [
        pkgs.libglvnd
        pkgs.wayland
      ];
    in
    ''
      export GBM_BACKENDS_PATH=${lib.makeSearchPathOutput "lib" "lib/gbm" mesa-drivers}
      export LIBGL_DRIVERS_PATH=${lib.makeSearchPathOutput "lib" "lib/dri" mesa-drivers}
      export LIBVA_DRIVERS_PATH=${lib.makeSearchPathOutput "out" "lib/dri" (mesa-drivers ++ vadrivers)}
      ${
        ''export __EGL_VENDOR_LIBRARY_FILENAMES=${mesa.drivers}/share/glvnd/egl_vendor.d/50_mesa.json"''${__EGL_VENDOR_LIBRARY_FILENAMES:+:$__EGL_VENDOR_LIBRARY_FILENAMES}"''
      }
      export LD_LIBRARY_PATH=${lib.makeLibraryPath mesa-drivers}:${
        lib.makeSearchPathOutput "lib" "lib/vdpau" libvdpau
      }:${lib.makeLibraryPath ld}"''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    '';
}

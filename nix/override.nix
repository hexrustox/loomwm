pkgs:
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
{
  buildInputs = with pkgs; [
    pkg-config
    libxkbcommon
  ];
  GBM_BACKENDS_PATH = "${lib.makeSearchPathOutput "lib" "lib/gbm" mesa-drivers}";
  LIBGL_DRIVERS_PATH = "${lib.makeSearchPathOutput "lib" "lib/dri" mesa-drivers}";
  LIBVA_DRIVERS_PATH = "${lib.makeSearchPathOutput "out" "lib/dri" (mesa-drivers ++ vadrivers)}";
  __EGL_VENDOR_LIBRARY_FILENAMES = "${mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
  LD_LIBRARY_PATH = "${lib.makeLibraryPath (mesa-drivers ++ ld)}:${
    lib.makeSearchPathOutput "lib" "lib/vdpau" libvdpau
  }";
}

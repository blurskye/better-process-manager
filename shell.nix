{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.clang
    pkgs.llvmPackages.libclang
    pkgs.glibc.dev
    pkgs.pkg-config
    pkgs.cmake
    # pkgs.rustc

    # pkgs.cargo
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
  BINDGEN_EXTRA_CLANG_ARGS = "--sysroot=${pkgs.glibc.dev}";
}

{}:
let
  rust-overlay = (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"));
  pkgs = (import <nixpkgs> {
    overlays = [ rust-overlay ];
  });
in
pkgs.mkShell {
  buildInputs = [
    (pkgs.rust-bin.stable.latest.rust.override {
      extensions = ["rust-src"];
    })

    pkgs.rustfmt
    pkgs.clippy
    
    pkgs.qt5.full
    pkgs.qtcreator


    # nodegui mini qt build requires this
    pkgs.p7zip

    pkgs.nodejs-14_x
    pkgs.yarn
    pkgs.sqlite
    pkgs.openssl

    # keep this line if you use bash
    pkgs.bashInteractive
  ];

  nativeBuildInputs = [
    pkgs.cmake
    pkgs.sccache
    pkgs.gcc
  ];

  RUSTC_WRAPPER = "sccache";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
}

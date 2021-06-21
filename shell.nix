# https://gutier.io/post/development-using-rust-with-nix/

{ nixpkgs ? import <nixpkgs> { }}:

let
  rustOverlay = builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
  pinnedPkgs = nixpkgs.fetchFromGitHub {
    owner  = "NixOS";
    repo   = "nixpkgs";
    rev    = "1fe6ed37fd9beb92afe90671c0c2a662a03463dd";
    sha256 = "1daa0y3p17shn9gibr321vx8vija6bfsb5zd7h4pxdbbwjkfq8n2";
  };
  pkgs = import pinnedPkgs {
    overlays = [ (import rustOverlay) ];
  };
in
  pkgs.mkShell {
    buildInputs = with pkgs; [
        # rust-bin.stable.latest.default
        (rust-bin.stable.latest.rust.override {
          extensions = ["rust-src"];
        })

        rust-analyzer

        rustfmt
        clippy
        
        qt5.full
        qtcreator


        # nodegui mini qt build requires this
        p7zip

        nodejs-14_x
        yarn
        sqlite
        openssl
        libGL

        # keep this line if you use bash
        bashInteractive
    ];

  nativeBuildInputs = [
    pkgs.cmake
    pkgs.sccache
    pkgs.gcc
  ];

  RUSTC_WRAPPER = "sccache";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";


    RUST_BACKTRACE = 1;
  }
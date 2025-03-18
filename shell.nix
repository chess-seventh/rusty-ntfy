# let
#   moz_overlay = import (builtins.fetchTarball "https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
#   nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
#   ruststable = (nixpkgs.latest.rustChannels.stable.rust.override { extensions = [ "rust-src" "rls-preview" "rust-analysis" "rustfmt-preview" ];});
# in
#   with nixpkgs;
# stdenv.mkDerivation {
#   name = "rust";
#   buildInputs = [ 
#     openssl 
#     rustup 
#     ruststable 
#     cmake 
#     zlib 
#     diesel-cli 
#     postgresql
#     cargo
#   ];
#
#   shellHook = ''
#         export OPENSSL_DIR="${openssl.dev}"
#         export OPENSSL_LIB_DIR="${openssl.out}/lib"
#         '';
# }

let
  # Last updated: 2/26/21. Update as necessary from https://status.nixos.org/...
  pkgs = import (fetchTarball("https://github.com/NixOS/nixpkgs/archive/da044451c6a70518db5b730fe277b70f494188f1.tar.gz")) {};

  # Rolling updates, not deterministic.
  # pkgs = import (fetchTarball("channel:nixpkgs-unstable")) {};
in pkgs.mkShell {
  buildInputs = [
    pkgs.cargo
    pkgs.rustc
    pkgs.rustfmt
    pkgs.diesel-cli
    pkgs.postgresql

    # Necessary for the openssl-sys crate:
    pkgs.openssl
    pkgs.pkg-config
  ];

  # See https://discourse.nixos.org/t/rust-src-not-found-and-other-misadventures-of-developing-rust-on-nixos/11570/3?u=samuela.
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}

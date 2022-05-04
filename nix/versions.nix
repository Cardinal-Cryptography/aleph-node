rec {
  rustToolchain =
    let
      # use Rust toolchain declared by the rust-toolchain file
      rustToolchain = with nixpkgs; overrideRustTarget ( rustChannelOf { rustToolchain = ../rust-toolchain; } );

      overrideRustTarget = rustChannel: rustChannel // {
        rust = rustChannel.rust.override {
          targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
        };
      };
    in
      rustToolchain;

  nixpkgs =
    let
      # this overlay allows us to use a version of the rust toolchain specified by the rust-toolchain file
      rustOverlay =
        import (builtins.fetchTarball {
          url = "https://github.com/mozilla/nixpkgs-mozilla/archive/f233fdc4ff6ba2ffeb1e3e3cd6d63bb1297d6996.tar.gz";
          sha256 = "1rzz03h0b38l5sg61rmfvzpbmbd5fn2jsi1ccvq22rb76s1nbh8i";
        });

      # pinned version of nix packages
      # main reason for not using here the newest available version at the time or writing is that this way we depend on glibc version 2.31 (Ubuntu 20.04 LTS)
      nixpkgs = import (builtins.fetchTarball {
        url = "https://github.com/NixOS/nixpkgs/archive/2c162d49cd5b979eb66ff1653aecaeaa01690fcc.tar.gz";
        sha256 = "08k7jy14rlpbb885x8dyds5pxr2h64mggfgil23vgyw6f1cn9kz6";
      }) { overlays = [
             rustOverlay
             # we override rust toolchain
             (self: super: {
               inherit (rustToolchain) cargo rust-src rust-std;
               rustc = rustToolchain.rust;
             })
           ];
         };
    in
      nixpkgs;

  llvm = nixpkgs.llvmPackages_11;

  stdenv = nixpkgs.keepDebugInfo llvm.stdenv;

  naersk =
    let
      naerskSrc = builtins.fetchTarball {
        url = "https://github.com/nix-community/naersk/archive/2fc8ce9d3c025d59fee349c1f80be9785049d653.tar.gz";
        sha256 = "1jhagazh69w7jfbrchhdss54salxc66ap1a1yd7xasc92vr0qsx4";
      };
    in
      nixpkgs.callPackage naerskSrc { inherit stdenv; cargo = rustToolchain.rust; rustc = rustToolchain.rust; };

  gitignore =
    let
      gitignoreSrc = nixpkgs.fetchFromGitHub {
        owner = "hercules-ci";
        repo = "gitignore.nix";
        rev = "5b9e0ff9d3b551234b4f3eb3983744fa354b17f1";
        sha256 = "o/BdVjNwcB6jOmzZjOH703BesSkkS5O7ej3xhyO8hAY=";
      };
    in
      import gitignoreSrc { inherit (nixpkgs) lib; };

  dockerNixpkgs =
    let
      dockerNixpkgs = builtins.fetchTarball {
        url = "https://github.com/NixOS/nixpkgs/archive/be872a7453a176df625c12190b8a6c10f6b21647.tar.gz";
        sha256 = "1hnwh2w5rhxgbp6c8illcrzh85ky81pyqx9309bkgpivyzjf2nba";
      };
    in
      import dockerNixpkgs {};
}

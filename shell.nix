let
  mozillaOverlay =
    import (builtins.fetchGit {
      url = "https://github.com/mozilla/nixpkgs-mozilla.git";
      rev = "f233fdc4ff6ba2ffeb1e3e3cd6d63bb1297d6996";
    });
  nixpkgs = import <nixpkgs> { overlays = [ mozillaOverlay ]; };
  rust-nightly = with nixpkgs; ((rustChannelOf { date = "2021-10-24"; channel = "nightly"; }).rust.override {
    extensions = [ "rust-src" ];
    targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
  });
  binutils-unwrapped' = nixpkgs.binutils-unwrapped.overrideAttrs (old: {
    name = "binutils-2.36.1";
    src = nixpkgs.fetchurl {
      url = "https://ftp.gnu.org/gnu/binutils/binutils-2.36.1.tar.xz";
      sha256 = "e81d9edf373f193af428a0f256674aea62a9d74dfe93f65192d4eae030b0f3b0";
    };
    patches = [];
  });
  llvm = nixpkgs.llvmPackages_13;
  llvmVersionString = "13.0.0";
  env = llvm.stdenv;
  cc = nixpkgs.wrapCCWith rec {
    cc = env.cc;
    bintools = nixpkgs.wrapBintoolsWith {
      bintools = binutils-unwrapped';
    };
  };
  customEnv = nixpkgs.overrideCC env cc;
in
with nixpkgs; customEnv.mkDerivation rec {
  name = "aleph-node";
  src = ./.;

  buildInputs = [
    llvm.clang
    binutils-unwrapped'
    openssl.dev
    pkg-config
    rust-nightly
    cacert
    protobuf
  ];

  shellHook = ''
    export RUST_SRC_PATH="${rust-nightly}/lib/rustlib/src/rust/src"
    export LIBCLANG_PATH="${llvm.libclang.lib}/lib"
    export PROTOC="${protobuf}/bin/protoc"
    export CFLAGS=" \
        ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
        $CFLAGS
    "
    export CXXFLAGS+=" \
        ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
        $CXXFLAGS
    "
    # From: https://github.com/NixOS/nixpkgs/blob/1fab95f5190d087e66a3502481e34e15d62090aa/pkgs/applications/networking/browsers/firefox/common.nix#L247-L253
    # Set C flags for Rust's bindgen program. Unlike ordinary C
    # compilation, bindgen does not invoke $CC directly. Instead it
    # uses LLVM's libclang. To make sure all necessary flags are
    # included we need to look in a few places.
    export BINDGEN_EXTRA_CLANG_ARGS=" \
        ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
        $BINDGEN_EXTRA_CLANG_ARGS
    "
    export RUSTFLAGS="-C target-cpu=x86-64-v3 $RUSTFLAGS"
    export CARGO_BUILD_TARGET="x86_64-unknown-linux-gnu"
  '';

  buildPhase = ''
    ${shellHook}
    export CARGO_HOME="$out/cargo"

    cargo build --locked --release -p aleph-node
  '';

  installPhase = ''
    mkdir -p $out/bin
    mv target/x86_64-unknown-linux-gnu/release/aleph-node $out/bin/
  '';
}

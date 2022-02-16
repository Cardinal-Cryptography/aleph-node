let
  # rustOverlay =
  #   import (builtins.fetchGit {
  #     url = "https://github.com/oxalica/rust-overlay.git";
  #     rev = "6d50a4f52d517a53cf740a9746f4e226ac17cf6a";
  #   });
  rustOverlay =
    import (builtins.fetchGit {
      url = "https://github.com/mozilla/nixpkgs-mozilla.git";
      rev = "f233fdc4ff6ba2ffeb1e3e3cd6d63bb1297d6996";
    });
  # nixpkgs = import (fetchTarball ("https://github.com/NixOS/nixpkgs/archive/66e44425c6dfecbea68a5d6dc221ccd56561d4f1.tar.gz")) { overlays = [ rustOverlay ]; };
  nixpkgs = import (builtins.fetchGit {
    url = "https://github.com/NixOS/nixpkgs.git";
    ref = "refs/tags/21.11";
  }) { overlays = [ rustOverlay ]; };
  # nixpkgs = import <nixpkgs> { overlays = [ rustOverlay ]; };
  rust-nightly = with nixpkgs; ((rustChannelOf { date = "2021-10-24"; channel = "nightly"; }).rust.override {
    extensions = [ "rust-src" ];
    targets = [ "x86_64-unknown-linux-musl" "wasm32-unknown-unknown" ];
  });
  # rust-nightly = nixpkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
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
  customGlibc = (import (builtins.fetchGit {
    # Descriptive name to make the store path easier to identify
    name = "glibc-old-revision";
    url = "https://github.com/NixOS/nixpkgs/";
    ref = "refs/tags/20.09";
    # ref = "refs/heads/nixos-20.09";
    # rev = "f6cc8cb29a3909136af1539848026bd41276e2ac";
     }) {}).glibc;
  # env = llvm.libcxxStdenv;
  env = llvm.stdenv;
  # env = nixpkgs.stdenvNoCC;
  cc = nixpkgs.wrapCCWith rec {
    cc = env.cc;
    bintools = nixpkgs.wrapBintoolsWith {
      bintools = binutils-unwrapped';
      # libc = nixpkgs.glibc_2.33-59;
      # libc = customGlibc;
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
    git
    findutils
    patchelf
    # customGlibc
    # glibc_2.31
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
    export CARGO_BUILD_TARGET="x86_64-unknown-linux-musl"
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

  fixupPhase = ''
    find $out -type f -exec patchelf --shrink-rpath '{}' \; -exec strip '{}' \; 2>/dev/null
  '';
}

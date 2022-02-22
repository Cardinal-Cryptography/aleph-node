let
  # this overlay allows us to use a specified version of the rust toolchain
  rustOverlay =
    import (builtins.fetchGit {
      url = "https://github.com/mozilla/nixpkgs-mozilla.git";
      rev = "f233fdc4ff6ba2ffeb1e3e3cd6d63bb1297d6996";
    });

  # pinned version of nix packages
  nixpkgs = import (builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/refs/tags/21.05.tar.gz";
    sha256 = "1ckzhh24mgz6jd1xhfgx0i9mijk6xjqxwsshnvq789xsavrmsc36";
  }) { overlays = [ rustOverlay ]; };

  # rustToolchain = nixpkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
  rustToolchain = with nixpkgs; ((rustChannelOf { rustToolchain = ./rust-toolchain; }).rust.override {
    targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
  });

  # rust toolchain requires a newer version of the linker than the one declared by nixpkgs
  binutils-unwrapped' = nixpkgs.binutils-unwrapped.overrideAttrs (old: {
    name = "binutils-2.36.1";
    src = nixpkgs.fetchurl {
      url = "https://ftp.gnu.org/gnu/binutils/binutils-2.36.1.tar.xz";
      sha256 = "e81d9edf373f193af428a0f256674aea62a9d74dfe93f65192d4eae030b0f3b0";
    };
    patches = [];
  });

  customRocksdb = nixpkgs.rocksdb.overrideAttrs ( _: {
    cmakeFlags = [
       "-DPORTABLE=0"
       "-DWITH_JNI=0"
       "-DWITH_BENCHMARK_TOOLS=0"
       "-DWITH_TESTS=1"
       "-DWITH_TOOLS=0"
       "-DWITH_BZ2=0"
       "-DWITH_LZ4=0"
       "-DWITH_SNAPPY=0"
       "-DWITH_ZLIB=0"
       "-DWITH_ZSTD=0"
       "-DWITH_GFLAGS=0"
       "-DUSE_RTTI=1"
       "-DFORCE_SSE42=1"
       "-DROCKSDB_BUILD_SHARED=0"
    ];

    propagatedBuildInputs = [];
  } );

  # declares a build environment where C and C++ compilers are delivered by the llvm/clang project
  # in this version build process should rely only on clang, without access to gcc
  llvm = nixpkgs.llvmPackages_12;
  env = llvm.stdenv;
  llvmVersionString = "${nixpkgs.lib.getVersion env.cc.cc}";
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
    rustToolchain
    llvm.clang
    binutils-unwrapped'
    openssl.dev
    protobuf
    customRocksdb
    pkg-config
    cacert
    git
    findutils
    patchelf
  ];

  shellHook = ''
    export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/src"
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
    export ROCKSDB_LIB_DIR="${customRocksdb}/lib"
    export ROCKSDB_STATIC=1
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

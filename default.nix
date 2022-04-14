{ release ? true
, crates ? { "aleph-node" = ["default"]; }
, rustflags ? "-C target-cpu=native"
, useCustomRocksDb ? false
, rocksDbOptions ? { version = "6.29.3";
                     useSnappy = false;
                     patchVerifyChecksum = true;
                     patchPath = ./nix/rocksdb.patch;
                     enableJemalloc = true;
                   }
}:
let
  # this overlay allows us to use a specified version of the rust toolchain
  rustOverlay =
    import (builtins.fetchTarball {
      url = "https://github.com/mozilla/nixpkgs-mozilla/archive/f233fdc4ff6ba2ffeb1e3e3cd6d63bb1297d6996.tar.gz";
      sha256 = "1rzz03h0b38l5sg61rmfvzpbmbd5fn2jsi1ccvq22rb76s1nbh8i";
    });

  overrideRustTarget = rustChannel: rustChannel // {
    rust = rustChannel.rust.override {
      targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
    };
  };
  # use Rust toolchain declared by the rust-toolchain file
  rustToolchain = with nixpkgs; overrideRustTarget ( rustChannelOf { rustToolchain = ./rust-toolchain; } );

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

  # declares a build environment where C and C++ compilers are delivered by the llvm/clang project
  # in this version build process should rely only on clang, without access to gcc
  llvm = nixpkgs.llvmPackages_11;
  env = nixpkgs.keepDebugInfo llvm.stdenv;
  llvmVersionString = "${nixpkgs.lib.getVersion env.cc.cc}";

  # we use a newer version of rocksdb than the one provided by nixpkgs
  # we disable all compression algorithms, force it to use SSE 4.2 cpu instruction set and disable its `verify_checksum` mechanism
  customRocksdb = nixpkgs.rocksdb.overrideAttrs (_: {

    src = builtins.fetchGit {
      url = "https://github.com/facebook/rocksdb.git";
      ref = "refs/tags/v${rocksDbOptions.version}";
    };

    version = "${rocksDbOptions.version}";

    patches = nixpkgs.lib.optional rocksDbOptions.patchVerifyChecksum rocksDbOptions.patchPath;

    cmakeFlags = [
       "-DPORTABLE=0"
       "-DWITH_JNI=0"
       "-DWITH_BENCHMARK_TOOLS=0"
       "-DWITH_TESTS=0"
       "-DWITH_TOOLS=0"
       "-DWITH_BZ2=0"
       "-DWITH_LZ4=0"
       "-DWITH_SNAPPY=${if rocksDbOptions.useSnappy then "1" else "0"}"
       "-DWITH_ZLIB=0"
       "-DWITH_ZSTD=0"
       "-DWITH_GFLAGS=0"
       "-DUSE_RTTI=0"
       "-DFORCE_SSE42=1"
       "-DROCKSDB_BUILD_SHARED=0"
       "-DWITH_JEMALLOC=${if rocksDbOptions.enableJemalloc then "1" else "0"}"
    ];

    propagatedBuildInputs = [];

    buildInputs = [nixpkgs.git] ++ nixpkgs.lib.optional rocksDbOptions.useSnappy nixpkgs.snappy ++ nixpkgs.lib.optional rocksDbOptions.enableJemalloc nixpkgs.jemalloc;
  });

  # allows to skip files listed by .gitignore
  # otherwise `nix-build` copies everything, including the target directory
  gitignoreSrc = nixpkgs.fetchFromGitHub {
    owner = "hercules-ci";
    repo = "gitignore.nix";
    rev = "5b9e0ff9d3b551234b4f3eb3983744fa354b17f1";
    sha256 = "o/BdVjNwcB6jOmzZjOH703BesSkkS5O7ej3xhyO8hAY=";
  };
  inherit (import gitignoreSrc { inherit (nixpkgs) lib; }) gitignoreSource;

  rocksDbShellHook = if useCustomRocksDb
                     then
                       "export ROCKSDB_LIB_DIR=${customRocksdb}/lib; export ROCKSDB_STATIC=1"
                     else "";

  naerskSrc = builtins.fetchTarball {
    url = "https://github.com/nix-community/naersk/archive/2fc8ce9d3c025d59fee349c1f80be9785049d653.tar.gz";
    sha256 = "1jhagazh69w7jfbrchhdss54salxc66ap1a1yd7xasc92vr0qsx4";
  };
  naersk = nixpkgs.callPackage naerskSrc { stdenv = env; cargo = rustToolchain.rust; rustc = rustToolchain.rust; };

  gitFolder = ./.git;
  gitCommitDrv = nixpkgs.runCommand "gitCommit" { nativeBuildInputs = [nixpkgs.git]; } ''
    cp -r ${gitFolder} ./.git
    echo $(git rev-parse --short HEAD) > $out
  '';
  gitCommit = builtins.readFile gitCommitDrv;

  pathToWasm = "target/" + (if release then "release" else "debug") + "/wbuild/aleph-runtime/aleph_runtime.compact.wasm";
  features =
    builtins.concatLists
      (builtins.attrValues
        (builtins.mapAttrs
          (package: features: builtins.map (feature: package + "/" + feature) features)
          crates
        )
      );
  enabledFeatures = nixpkgs.lib.concatStringsSep "," features;
  featuresFlag = if enabledFeatures == "" then "" else "--features " + enabledFeatures;
  packages = nixpkgs.lib.concatStringsSep "," (builtins.attrNames crates);
  packagesFlag = if packages == "" then "" else "--package " + packages;
in
with nixpkgs; naersk.buildPackage rec {
  name = "aleph-node";
  src = gitignoreSource ./.;
  inherit release;
  nativeBuildInputs = [
    cacert
    git
    rustToolchain.cargo
    rustToolchain.rustc
    pkg-config
  ];
  buildInputs = [
    openssl.dev
    protobuf
  ] ++ nixpkgs.lib.optional useCustomRocksDb customRocksdb;
  cargoBuildOptions = opts:
    [packagesFlag featuresFlag]
    ++ ["--locked"]
    ++ opts;
  shellHook = ''
    ${rocksDbShellHook}
    export SUBSTRATE_CLI_GIT_COMMIT_HASH=${SUBSTRATE_CLI_GIT_COMMIT_HASH}
    export RUSTFLAGS="${rustflags}"
    export LIBCLANG_PATH=${LIBCLANG_PATH};
    export PROTOC=${PROTOC}
    export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS}"
    export CFLAGS="${CFLAGS}"
    export CXXFLAGS="${CXXFLAGS}"
  '';
  preBuild = ''
    ${shellHook}
  '';
  postInstall = ''
    if [ -f ${pathToWasm} ]; then
      mkdir -p $out/lib
      cp ${pathToWasm} $out/lib/
    fi
  '';

  SUBSTRATE_CLI_GIT_COMMIT_HASH="${gitCommit}";
  RUSTFLAGS="${rustflags}";
  ROCKSDB_STATIC=1;
  LIBCLANG_PATH="${llvm.libclang.lib}/lib";
  PROTOC="${protobuf}/bin/protoc";
  # From: https://github.com/NixOS/nixpkgs/blob/1fab95f5190d087e66a3502481e34e15d62090aa/pkgs/applications/networking/browsers/firefox/common.nix#L247-L253
  # Set C flags for Rust's bindgen program. Unlike ordinary C
  # compilation, bindgen does not invoke $CC directly. Instead it
  # uses LLVM's libclang. To make sure all necessary flags are
  # included we need to look in a few places.
  BINDGEN_EXTRA_CLANG_ARGS=" \
     ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
  ";
  CFLAGS=" \
    ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
  ";
  CXXFLAGS=" \
    ${"-isystem ${llvm.libclang.lib}/lib/clang/${llvmVersionString}/include"} \
  ";
}

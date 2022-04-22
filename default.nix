{ versions ? import ./nix/versions.nix
, release ? true
, name ? "aleph-node"
, crates ? { "aleph-node" = []; }
, runTests ? false
, singleStep ? false
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
  nixpkgs = versions.nixpkgs;
  rustToolchain = versions.rustToolchain;
  naersk = versions.naersk;

  # declares a build environment where C and C++ compilers are delivered by the llvm/clang project
  # in this version build process should rely only on clang, without access to gcc
  llvm = versions.llvm;
  env = versions.stdenv;
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

    buildInputs = nixpkgs.lib.optionals rocksDbOptions.useSnappy [nixpkgs.snappy] ++
                  nixpkgs.lib.optionals rocksDbOptions.enableJemalloc [nixpkgs.jemalloc] ++
                 [nixpkgs.git];
  });
  rocksDbShellHook = if useCustomRocksDb
                     then
                       "export ROCKSDB_LIB_DIR=${customRocksdb}/lib; export ROCKSDB_STATIC=1"
                     else "";

  # newer versions of substrate support providing a version hash by means of an env variable, i.e. SUBSTRATE_CLI_GIT_COMMIT_HASH
  gitFolder = ./.git;
  gitCommitDrv = nixpkgs.runCommand "gitCommit" { nativeBuildInputs = [nixpkgs.git]; } ''
    cp -r ${gitFolder} ./.git
    echo $(git rev-parse --short HEAD) > $out
  '';
  gitCommit = builtins.readFile gitCommitDrv;

  modePath = if release then "release" else "debug";
  pathToWasm = "target/" + modePath + "/wbuild/aleph-runtime/target/wasm32-unknown-unknown/" + modePath + "/aleph_runtime.wasm";
  pathToCompactWasm = "target/" + modePath + "/wbuild/aleph-runtime/aleph_runtime.compact.wasm";

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
  packageFlags = builtins.map (crate: "--package " + crate) (builtins.attrNames crates);

  # allows to skip files listed by .gitignore
  # otherwise `nix-build` copies everything, including the target directory
  inherit (versions.gitignore) gitignoreFilter;
  # we need to include the .git directory, since Substrate build scripts use git to retrieve commit hash of HEAD
  gitFilter = src:
    let
      srcIgnored = gitignoreFilter src;
    in
      path: type:
        builtins.baseNameOf path == ".git" || srcIgnored path type;
  src = nixpkgs.lib.cleanSourceWith {
    src = ./.;
    filter = gitFilter ./.;
    name = "aleph-source";
  };
in
with nixpkgs; naersk.buildPackage rec {
  inherit name release src singleStep;
  doCheck = runTests;
  nativeBuildInputs = [
    git
    cacert
    pkg-config
    llvm.libclang
  ];
  buildInputs = [
    openssl.dev
    protobuf
  ] ++ nixpkgs.lib.optional useCustomRocksDb customRocksdb;
  cargoBuildOptions = opts:
    packageFlags
    ++ [featuresFlag]
    ++ ["--locked" "--offline"]
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
    if [ -f ${pathToCompactWasm} ]; then
      mkdir -p $out/lib
      cp ${pathToCompactWasm} $out/lib/
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

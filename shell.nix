{ versions ? import ./nix/versions.nix {}
, nixpkgs ? versions.nixpkgs
, useCustomRocksdb ? false
}:
let
  # declares a build environment where C and C++ compilers are delivered by the llvm/clang project
  # in this version build process should rely only on clang, without access to gcc
  llvm = nixpkgs.llvmPackages_11;
  env = llvm.stdenv;
  llvmVersionString = "${nixpkgs.lib.getVersion env.cc.cc}";

  inherit (versions) customRocksdb;
in
with nixpkgs; mkShell.override { stdenv = env; } {
  nativeBuildInputs = [
    cargo
    rustc
    llvm.clang
    openssl.dev
    protobuf
    pkg-config
    cacert
    git
    findutils
    patchelf
    crate2nix
  ] ++ nixpkgs.lib.optional useCustomRocksdb versions.customRocksdb;

  shellHook = ''
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
    ${nixpkgs.lib.optionalString useCustomRocksdb "export ROCKSDB_LIB_DIR=${customRocksdb}/lib"}
    ${nixpkgs.lib.optionalString useCustomRocksdb "export ROCKSDB_STATIC=1"}
  '';
}

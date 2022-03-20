{ versions ? import ./versions.nix {}
, nixpkgs ? versions.nixpkgs
, gitignoreSource ? versions.gitignoreSource
, customRocksdb ? versions.customRocksdb
}:
let
  # declares a build environment where C and C++ compilers are delivered by the llvm/clang project
  # in this version build process should rely only on clang, without access to gcc
  llvm = nixpkgs.llvmPackages_11;
  env = llvm.stdenv;
  llvmVersionString = "${nixpkgs.lib.getVersion env.cc.cc}";

  # allows to skip files listed by .gitignore
  # otherwise `nix-build` copies everything, including the target directory
  src = gitignoreSource ../.;

  # we need this to generate nix-based build plan
  crate2nix = nixpkgs.crate2nix;
  inherit (import ./tools.nix { pkgs = nixpkgs; lib = nixpkgs.lib; stdenv = env; inherit crate2nix; }) generatedCargoNix vendoredCargoLock;

  # some of our dependencies requires external libraries like protobuf, etc.
  customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
    stdenv = env;
    defaultCrateOverrides = pkgs.defaultCrateOverrides // (
      let protobufFix = _: {
            # provides env variables necessary to use protobuf during compilation
            buildInputs = [ pkgs.protobuf ];
            PROTOC="${pkgs.protobuf}/bin/protoc";
          };
      in rec {
        librocksdb-sys = _: {
          buildInputs = [customRocksdb];
          LIBCLANG_PATH="${llvm.libclang.lib}/lib";
          ROCKSDB_LIB_DIR="${customRocksdb}/lib";
          # forces librocksdb-sys to statically compile with our customRocksdb
          ROCKSDB_STATIC=1;
        };
        libp2p-core = protobufFix;
        libp2p-plaintext = protobufFix;
        libp2p-floodsub = protobufFix;
        libp2p-gossipsub = protobufFix;
        libp2p-identify = protobufFix;
        libp2p-kad = protobufFix;
        libp2p-relay = protobufFix;
        libp2p-rendezvous = protobufFix;
        libp2p-noise = protobufFix;
        sc-network = protobufFix;
        substrate-test-runtime = attrs:
          let
            substrateSrc = attrs.src;
            # we need to merge these two vendored CARGO_HOMEs
            vendoredSubstrateCargo = vendoredCargoLock "${substrateSrc}" "Cargo.lock";
            vendoredCargo = vendoredCargoLock "${src}" "Cargo.lock";
            CARGO_HOME="$out/.cargo";
            wrappedCargo = pkgs.writeShellScriptBin "cargo" ''
               export CARGO_HOME="${CARGO_HOME}"
               exec ${pkgs.cargo}/bin/cargo "$@"
            '';
          in
          {
            inherit CARGO_HOME;
            buildInputs = [pkgs.git pkgs.cacert];
            CARGO = "${wrappedCargo}/bin/cargo";
            preConfigure = ''
              # populate vendored CARGO_HOME
              mkdir -p $out
              mkdir -p $out/cargo-vendor-dir
              for d in ${vendoredCargo}/*/ ; do
                ln -sf $d $out/cargo-vendor-dir/
              done
              for d in ${vendoredSubstrateCargo}/*/ ; do
                ln -sf $d $out/cargo-vendor-dir/
              done
              ln -s ${vendoredSubstrateCargo}/.cargo ${CARGO_HOME}
            '';
            postBuild = ''
              # we need to clean after ourselves
              # buildRustCrate derivation will populate it with necessary artifacts
              rm -rf $out
            '';
          };
        prost-build = protobufFix;
        aleph-runtime = _:
          # this is a bit tricky - aleph-runtime's build.rs calls Cargo, so we need to provide it a populated
          # CARGO_HOME, otherwise it tries to download crates (it doesn't work with sandboxed nix-build)
          let
            vendoredCargo = vendoredCargoLock "${src}" "Cargo.lock";
            CARGO_HOME="$out/.cargo";
            # this way Cargo called by build.rs can see our vendored CARGO_HOME
            wrappedCargo = pkgs.writeShellScriptBin "cargo" ''
               export CARGO_HOME="${CARGO_HOME}"
               exec ${pkgs.cargo}/bin/cargo "$@"
            '';
          in
          {
            inherit src CARGO_HOME;
            # otherwise it has no access to other dependencies in our workspace
            workspace_member = "bin/runtime";
            buildInputs = [pkgs.git pkgs.cacert];
            CARGO = "${wrappedCargo}/bin/cargo";
            # build.rs is called during `configure` phase, so we need to setup during `preConfigure`
            preConfigure = ''
              # populate vendored CARGO_HOME
              mkdir -p $out
              ln -s ${vendoredCargo}/.cargo ${CARGO_HOME}
              ln -s ${vendoredCargo} $out/cargo-vendor-dir
              ln -s ${vendoredCargo}/Cargo.lock $out/Cargo.lock
            '';
            postBuild = ''
              # we need to clean after ourselves
              # buildRustCrate derivation will populate it with necessary artifacts
              rm -rf $out
            '';
          };
    }
    );
  };

  generated = generatedCargoNix {
    name = "aleph-node";
    inherit src;
  };
  project = import generated { pkgs = nixpkgs; buildRustCrateForPkgs = customBuildRustCrateForPkgs; };
in
{ inherit project src; }

{ buildOptions ? {}
, rustToolchainFile ? ./rust-toolchain
}:
let
  versions = import ./nix/versions.nix { inherit rustToolchainFile; };
  nixpkgs = versions.nixpkgs;
  env = versions.stdenv;
  project = import ./default.nix ( buildOptions // { inherit versions; } );
  rust = versions.rustToolchain.rust.override {
    extensions = [ "rust-src" ];
  };
  # nativeBuildInputs = [rust nixpkgs.cacert nixpkgs.openssl] ++ project.nativeBuildInputs;
  nativeBuildInputs = [nixpkgs.nodejs nixpkgs.pkg-config nixpkgs.cacert nixpkgs.openssl] ++ project.nativeBuildInputs;

  # nativeBuildInputs = [nixpkgs.pkg-config nixpkgs.zlib nixpkgs.cacert nixpkgs.openssl];
  fixedNativeBuildInputs = nixpkgs.lib.lists.remove versions.rustToolchain.rust nativeBuildInputs;
  lengthBefore = nixpkgs.lib.lists.length nativeBuildInputs;
  # lengthAfter = nixpkgs.lib.lists.length fixedNativeBuildInputs;
in
# builtins.trace (nixpkgs.lib.lists.forEach nativeBuildInputs (x: builtins.trace x x)) (nixpkgs.mkShell.override { stdenv = env; }
builtins.trace (lengthBefore) (
nixpkgs.mkShell.override { stdenv = env; }
  {
    # inherit nativeBuildInputs;
    # inherit fixedNativeBuildInputs;
    nativeBuildInputs = fixedNativeBuildInputs;
    inherit (project) buildInputs shellHook;
    # RUST_SRC_PATH might be needed by the `rust-analyzer`
    # RUST_SRC_PATH = "${versions.rustToolchain.rust-src}/lib/rustlib/src/rust/library/";
  }
)

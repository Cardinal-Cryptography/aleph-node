{ buildOptions ? {} }:
let
  versions = import ./nix/versions.nix;
  nixpkgs = versions.nixpkgs;
  env = versions.stdenv;
  project = import ./default.nix buildOptions ;
  rust = nixpkgs.rust.override {
    extensions = [ "rust-src" ];
  };
  nativeBuildInputs = [rust nixpkgs.cacert] ++ project.nativeBuildInputs;
in
nixpkgs.mkShell.override { stdenv = env; }
  {
    inherit nativeBuildInputs;
    inherit (project) buildInputs shellHook;
  }

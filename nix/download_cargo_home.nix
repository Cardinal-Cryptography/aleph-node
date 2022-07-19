{ versions ? import ./versions.nix {} }:
let
  nixpkgs = versions.nixpkgs;
  rustToolchain = versions.rustToolchain;
  src = nixpkgs.lib.sourceFilesBySuffices ./.. [ ".toml" ".lock" "main.rs" "lib.rs" ];
  # leave only .toml, .lock and empty main.rs and lib.rs files
  # this allows to cache CARGO_HOME on per Cargo.lock basis
  fixedSrc = nixpkgs.runCommand "cleanedSrc" {} ''
    mkdir -p $out
    cp -a ${src}/. $out/
    find $out/ -name "*.rs" | xargs -I {} sh -c "echo -n "" > {}"
  '';
in
nixpkgs.runCommand "cargoFetch" { nativeBuildInputs = [rustToolchain.rust nixpkgs.cacert]; } ''
  mkdir -p $out
  CARGO_HOME=$out cargo fetch --locked --manifest-path ${fixedSrc}/Cargo.toml
''

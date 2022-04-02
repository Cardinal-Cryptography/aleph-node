{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, features ? [ "default" ]
, targetFeatures ? import ./nix/target-features.nix
, useCustomRocksdb ? false
}:
let
  crates ={ "aleph-node" = ["short_session"]; } ;
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions targetFeatures useCustomRocksdb; }).project;
  alephDerivation = alephNode.workspaceMembers.aleph-node.build.override { inherit runTests features; };
  alephRuntimeDerivation = builtins.head (builtins.filter (dep: dep.crateName == "aleph-runtime") alephDerivation.dependencies);
in
nixpkgs.symlinkJoin {
  name = "aleph-node_with_aleph-runtime";
  paths = [ alephDerivation alephRuntimeDerivation ];
}

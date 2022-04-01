{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, features ? ["default"]
, targetFeatures ? import ./nix/target-features.nix
, useCustomRocksdb ? false
}:
let
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions targetFeatures useCustomRocksdb; }).project;
  workspaceMembers = builtins.mapAttrs (_: crate: crate.build.override { inherit runTests; }) alephNode.workspaceMembers;
  alephDerivation = workspaceMembers."aleph-node".override { inherit features; };
  alephRuntimeDerivation = builtins.head (builtins.filter (crate: crate.crateName == "aleph-runtime") alephDerivation.dependencies);
in
nixpkgs.symlinkJoin {
  name = "aleph-node-with-runtime";
  paths = [alephDerivation alephRuntimeDerivation];
}

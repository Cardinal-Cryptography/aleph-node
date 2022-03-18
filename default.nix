{ rocksDBVersion ? "6.29.3" }:
let
  aleph-node = ( import ./nix/aleph-node.nix { inherit rocksDBVersion; } ).project;
in
aleph-node.workspaceMembers."aleph-node".build

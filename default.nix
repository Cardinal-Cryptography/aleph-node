{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, allCrates ? false
}:
let
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions; }).project;
  workspaceMembers = builtins.mapAttrs (_: crate: crate.build.override { inherit runTests; }) alephNode.workspaceMembers;
  allWorkspaceMembers = nixpkgs.symlinkJoin {
      name = "all-workspace-members";
      paths = builtins.attrValues workspaceMembers;
  };
in
if allCrates then
  allWorkspaceMembers
else
  workspaceMembers."aleph-node".build

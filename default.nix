{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, crates ? { "aleph-node" = ["default"]; }
, targetFeatures ? import ./nix/target-features.nix
, useCustomRocksdb ? false
}:
let
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions targetFeatures useCustomRocksdb; }).project;
  workspaceMembers = builtins.mapAttrs (_: crate: crate.build.override { inherit runTests; }) alephNode.workspaceMembers;
  filteredWorkspaceMembers =
    if crates == [] then
      builtins.attrValues workspaceMembers
    else
      builtins.attrValues (builtins.mapAttrs (crate: features: (builtins.getAttr crate workspaceMembers).override { inherit features; }) crates);
  workspaceMembersToBuild =
    if builtins.length filteredWorkspaceMembers == 1 then
      builtins.head filteredWorkspaceMembers
    else
      nixpkgs.symlinkJoin {
        name = "filtered-workspace-members";
        paths = filteredWorkspaceMembers;
      };
in
workspaceMembersToBuild

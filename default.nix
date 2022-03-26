{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, crates ? [ "aleph-node" ]
, targetFeatures ? import ./nix/target-features.nix
}:
let
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions targetFeatures; }).project;
  workspaceMembers = builtins.mapAttrs (_: crate: crate.build.override { inherit runTests; }) alephNode.workspaceMembers;
  filteredWorkspaceMembers =
    if crates == [] then
      builtins.attrValues workspaceMembers
    else
      builtins.map (crate: builtins.getAttr crate workspaceMembers) (nixpkgs.lib.unique crates);
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

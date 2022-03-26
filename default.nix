{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, crates ? [ "aleph-node" ]
  # default set of cpu features for x86-64-v3 (rustc --print=cfg -C target-cpu=x86-64-v3)
, targetFeatures ? [ "avx" "avx2" "bmi1" "bmi2" "fma" "fxsr" "lzcnt" "popcnt" "sse" "sse2" "sse3" "sse4.1" "sse4.2" "ssse3" "xsave" ]
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

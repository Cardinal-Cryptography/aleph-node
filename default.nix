{ nixpkgs ? (import ./nix/versions.nix {}).nixpkgs
, rocksDBVersion ? "6.29.3"
, runTests ? false
, crates ? [ "aleph-node" ]
}:
let
  versions = import ./nix/versions.nix { inherit rocksDBVersion; };
  alephNode = (import ./nix/aleph-node.nix { inherit versions; }).project;
  workspaceMembers = builtins.mapAttrs (_: crate: crate.build.override { inherit runTests; extraRustcOpts = "--target x86-64-v3"; }) alephNode.workspaceMembers;
  cratesFilter =
    let
      cratesAttrs = builtins.listToAttrs (builtins.map (member: { name = member; value = null; }) workspaceMembers);
    in
    n: builtins.hasAttr n cratesAttrs;
  filteredWorkspaceMembers = nixpkgs.lib.filterAttrs (n: _: cratesFilter n) workspaceMembers;
  workspaceMembersToBuild =
    if crates == [] then
      nixpkgs.symlinkJoin {
        name = "all-workspace-members";
        paths = builtins.attrValues workspaceMembers;
      }
    else
      if builtins.length crates == 1 then
        let
          crateName = builtins.head crates;
        in
        workspaceMembers."${crateName}"
      else
        nixpkgs.symlinkJoin {
          name = "filtered-workspace-members";
          paths = builtins.attrValues filteredWorkspaceMembers;
        };
in
workspaceMembersToBuild

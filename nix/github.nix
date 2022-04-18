{ nixpkgs ? (import ./versions.nix).nixpkgs }:
let
  alephNodeDrv = import ../default.nix;
  alephNode = alephNodeDrv { crates = { "aleph-node" = []; "aleph-runtime" = []; }; };
  alephRuntime = alephNodeDrv { crates = { "aleph-runtime" = []; }; };
  alephNodeShortSession = alephNodeDrv { crates = { "aleph-node" = ["short_session"]; "aleph-runtime" = ["short_session"]; }; };
in
[ nixpkgs.patchelf alephNode alephRuntime alephNodeShortSession ]

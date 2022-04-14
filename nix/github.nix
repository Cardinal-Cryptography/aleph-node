{ nixpkgs ? (import ./versions.nix).nixpkgs }:
let
  alephNodeDrv = import ../default.nix;
  alephNode = alephNodeDrv { crates = { "aleph-node" = ["default"]; }; };
  alephNodeShortSession = alephNodeDrv { crates = { "aleph-node" = ["short_session"]; }; };
in
[ nixpkgs.patchelf alephNode alephNodeShortSession ]

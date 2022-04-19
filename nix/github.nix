let
  nixpkgs = (import ./versions.nix).nixpkgs;
  alephNodeDrv = import ../default.nix;
  alephNode = alephNodeDrv { crates = { "aleph-node" = []; }; };
  alephRuntime = alephNodeDrv { crates = { "aleph-runtime" = []; }; };
  alephNodeShortSession = alephNodeDrv { crates = { "aleph-node" = ["short_session"]; "aleph-runtime" = ["short_session"]; }; };
in
[ nixpkgs.patchelf alephNode alephRuntime alephNodeShortSession ]

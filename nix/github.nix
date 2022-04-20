let
  nixpkgs = (import ./versions.nix).nixpkgs;
  alephNodeDrv = import ../default.nix;
  alephNode = alephNodeDrv { crates = { "aleph-node" = []; }; name = "aleph-node"; };
  alephRuntime = (alephNodeDrv {
    crates = { "aleph-runtime" = []; };
    name = "aleph-runtime";
  }).overrideAttrs (
    attrs: {
      meta.priority = alephNode.meta.priority + 1;
    }
  );
  alephNodeShortSession = (alephNodeDrv {
    crates = { "aleph-node" = ["short_session"]; "aleph-runtime" = ["short_session"]; };
    name = "short_session";
  }).overrideAttrs (
      attrs: {
        meta.priority = alephRuntime.meta.priority + 1;
      }
    );
in
[ nixpkgs.patchelf alephNode alephRuntime alephNodeShortSession ]

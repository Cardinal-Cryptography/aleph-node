let
  nixpkgs = (import ./versions.nix).nixpkgs;
  alephNodeDrv = import ../default.nix;
  alephNode = nixpkgs.lib.hiPrio (
    alephNodeDrv {
      crates = { "aleph-node" = []; };
      name = "aleph-node";
    }
  );
  alephRuntime = nixpkgs.lib.setPrio (alephNode.meta.priority + 1) (
    alephNodeDrv {
      crates = { "aleph-runtime" = []; };
      name = "aleph-runtime";
    }
  );
  alephNodeShortSession = nixpkgs.lib.setPrio (alephRuntime.meta.priority + 1) (
    alephNodeDrv {
      crates = { "aleph-node" = ["short_session"]; "aleph-runtime" = ["short_session"]; };
      name = "short_session";
    }
  );
in
[ nixpkgs.patchelf alephNode alephRuntime alephNodeShortSession ]

{ versions ? import ./nix/versions.nix
, useCustomRocksDb ? false
, rocksDbOptions ? { version = "6.29.3";
                     useSnappy = false;
                     patchVerifyChecksum = true;
                     patchPath = ./nix/rocksdb.patch;
                     enableJemalloc = true;
                   }
}:
let
  nixpkgs = versions.nixpkgs;
  env = versions.stdenv;
  project = import ./default.nix { inherit versions useCustomRocksDb rocksDbOptions; };
in
nixpkgs.mkShell.override { stdenv = env; }
  {
    inherit (project) nativeBuildInputs buildInputs shellHook;
  }

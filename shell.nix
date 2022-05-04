{ useCustomRocksDb ? false
, rocksDbOptions ? { version = "6.29.3";
                     useSnappy = false;
                     patchVerifyChecksum = true;
                     patchPath = ./nix/rocksdb.patch;
                     enableJemalloc = true;
                   }
}:
let
  versions = import ./nix/versions.nix;
  nixpkgs = versions.nixpkgs;
  env = versions.stdenv;
  project = import ./default.nix { inherit useCustomRocksDb rocksDbOptions; };
  nativeBuildInputs = project.nativeBuildInputs ++ [nixpkgs.cacert];
in
nixpkgs.mkShell.override { stdenv = env; }
  {
    inherit nativeBuildInputs;
    inherit (project) buildInputs shellHook;
  }

{ versions ? import ./versions.nix {}
, nixpkgs ? versions.nixpkgs
}:
[ nixpkgs.patchelf ]

{ nixpkgs ? (import ./versions.nix).nixpkgs }:
[ nixpkgs.patchelf ]

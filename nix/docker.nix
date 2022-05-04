# defines a derivation that builds a minimal docker image containing aleph-node and its src folder
{
  alephArgs ? {}
}:
let
  versions = import ./versions.nix;
  nixpkgs = versions.nixpkgs;
  nixpkgsForDocker = versions.dockerNixpkgs;

  alephNode = import ../default.nix alephArgs;
  dockerEntrypointScript = (nixpkgs.writeScriptBin "docker_entrypoint.sh" (builtins.readFile ../docker/docker_entrypoint.sh)).overrideAttrs(old: {
    buildCommand = ''
      ${old.buildCommand}
      # fixes #! /usr/bin/env bash preamble
      patchShebangs $out
    '';
  });
in
nixpkgsForDocker.dockerTools.buildImage {
  name = "aleph-node";
  created = "now";
  contents = [alephNode dockerEntrypointScript nixpkgs.bash nixpkgs.coreutils];
  config = {
    Env = [
      "PATH=${alephNode}/bin:${dockerEntrypointScript}/bin:${nixpkgs.bash}/bin:${nixpkgs.coreutils}/bin"
    ];
    Entrypoint = "${dockerEntrypointScript}/bin/docker_entrypoint.sh";
    ExposedPorts = {
      "30333" = {};
      "9933" = {};
      "9944" = {};
    };
  };
}

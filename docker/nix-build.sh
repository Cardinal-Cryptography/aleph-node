#!/usr/bin/env bash
set -euo pipefail

SPAWN_SHELL=${SPAWN_SHELL:-false}
SHELL_NIX_FILE=${SHELL_NIX_FILE:-"shell.nix"}

while getopts "s" flag
do
    case "${flag}" in
        s) SPAWN_SHELL=true;;
        *)
            usage
            exit
            ;;
    esac
done

function usage(){
    echo "Usage:
      ./nix-build.sh [-s - spawn nix-shell]"
}

if [ $SPAWN_SHELL = true ]
then
    nix-shell --pure $SHELL_NIX_FILE
else
    nix-build $SHELL_NIX_FILE
    patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 ./result/bin/aleph-node
    mv ./result/bin/aleph-node ./
fi

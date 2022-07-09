#!/usr/bin/env bash
set -euo pipefail

SPAWN_SHELL=${SPAWN_SHELL:-false}
SHELL_NIX_FILE=${SHELL_NIX_FILE:-"default.nix"}
DYNAMIC_LINKER_PATH=${DYNAMIC_LINKER_PATH:-"/lib64/ld-linux-x86-64.so.2"}
CRATES=${CRATES:-'{ "aleph-node" = []; }'}
SINGLE_STEP=${SINGLE_STEP:-'false'}
RUSTFLAGS=${RUSTFLAGS:-'"-C target-cpu=generic"'}
if [ -z ${PATH_TO_FIX+x} ]; then
    PATH_TO_FIX="result/bin/aleph-node"
fi

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
    ARGS=(--arg crates "${CRATES}" --arg singleStep "${SINGLE_STEP}" --arg rustflags "${RUSTFLAGS}")

    # first we download all dependencies
    echo fetching depedencies...
    CARGO_HOME="$(realpath ~/cargo-home)"
    nix-shell --pure --run "CARGO_HOME=$CARGO_HOME cargo fetch --locked --offline || CARGO_HOME=$CARGO_HOME cargo fetch --locked"

    echo building...
    nix-build --max-jobs auto --arg cargoHomePath "$CARGO_HOME" --show-trace $SHELL_NIX_FILE "${ARGS[@]}"
    echo build finished

    echo copying results...
    cp -Lr result result.copied
    mv result result.tmp
    mv result.copied result
    rm result.tmp
    echo results copied

    # we need to change the dynamic linker
    # otherwise our binary references one that is specific for nix
    # we need it for aleph-node to be run outside nix-shell
    if [[ ! -z "$PATH_TO_FIX" && -f $PATH_TO_FIX ]]; then
        echo patching...
        chmod +w $PATH_TO_FIX
        patchelf --set-interpreter $DYNAMIC_LINKER_PATH $PATH_TO_FIX
    fi
    echo nix-build finished
fi

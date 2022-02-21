#!/bin/bash

set -e

mpb="DEFAULT_MILLISECS_PER_BLOCK: u64 ="
sp="DEFAULT_SESSION_PERIOD: u32 ="
spe="DEFAULT_SESSIONS_PER_ERA: SessionIndex ="
path="primitives/src/lib.rs"

if [ -n "$1" ]; then
    sed -i "s/$mpb .*;/$mpb $1;/" $path
fi
if [ -n "$2" ]; then
    sed -i "s/$sp .*;/$sp $2;/" $path
fi
if [ -n "$3" ]; then
    sed -i "s/$spe .*;/$spe $3;/" $path
fi

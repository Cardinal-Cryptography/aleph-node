#!/usr/bin/env bash

set -euo pipefail

function usage(){
  cat << EOF
Substitutes the branch name of the repository Cardinal-Cryptography/substrate.git in all Cargo.toml files.

Usage:
  $0 <new_branch_name>
EOF
  exit 0
}

BRANCH="${1:-}"
if [[ -z "${BRANCH}" ]]; then
       usage
       exit 2
fi

# Find all `Cargo.toml` files outside any `target` directory.
paths=$(find . -mindepth 2 -type f -name "Cargo.toml" -not -path "*/target/*") || echo "Problems with finding Cargo.toml files"

for path in ${paths[@]}; do
    echo "Upgrading ${path}"
    # 1. Find and capture `Cardinal-Cryptography/substrate.git", branch = "` substring. It will be available as `\1`. In place
    #    of spaces there can be sequence of `\s` characters.
    # 2. Find and capture whatever is after closing `"` and before `,` or `}`. It will be available as `\2`.
    # 3. Substitute new branch and concatenate it with `\1` and `\2`.

    sed -e '/Cardinal-Cryptography\/substrate.git/s/\(branch\s*=\s*"\)[^"]*"\([^,}]*\)/\1'"${BRANCH}"'"\2/' < $path > x
    mv x "${path}"
done

exit 0

#!/usr/bin/env bash
../generate_interface.sh

diff -y -W 200 --suppress-common-lines aleph_zero.rs aleph-node/aleph-client/src/aleph_zero.rs
diff_exit_code=$?
if [[ ! $diff_exit_code -eq 0 ]]; then
  echo "Current runtime metadata is different than versioned in git!"
  echo "Run ./generate_interface.sh from aleph-client directory and commit to git."
   exit 1
fi
echo "Current runtime metadata and versioned in git matches."

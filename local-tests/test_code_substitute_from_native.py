#!/bin/env python
import os
import os.path

from code_substitute_utils import *

SEND_RUNTIME = 'send-runtime/target/release/send_runtime'

CORRUPTED_BINARY = 'test-code-substitute/build/aleph-node'
FIXING_RUNTIME = 'test-code-substitute/build/fixing_runtime.wasm'

NODES = 4
WORKDIR = '.'
PHRASES = ['//Alice', '//Bob', '//Cedric', '//Dick']


def check_if_files_are_built():
    assert os.path.isfile(CORRUPTED_BINARY), 'Corrupted binary is not ready'
    assert os.path.isfile(FIXING_RUNTIME), 'Fixing runtime is not ready'


def test_code_substitute():
    check_if_files_are_built()

    chain = run_binary(WORKDIR, CORRUPTED_BINARY, PHRASES, 'corrupted')
    query_runtime_version(chain)
    check_highest(chain)

    stalled_hash, finalized = wait_for_stalling(chain)

    update_chainspec(stalled_hash, FIXING_RUNTIME)
    restart_nodes(chain)

    wait_for_continuation(chain, finalized)
    assert False, 'Should have panicked'


if __name__ == '__main__':
    test_code_substitute()

#!/bin/env python
import json
import os
import os.path
import subprocess

from code_substitute_utils import *

SEND_RUNTIME = 'send-runtime/target/release/send_runtime'

BINARY = 'test-code-substitute/build/aleph-node'
CORRUPTED_RUNTIME = 'test-code-substitute/build/corrupted_runtime.wasm'
FIXING_RUNTIME = 'test-code-substitute/build/fixing_runtime.wasm'
NEW_RUNTIME = 'test-code-substitute/build/new_runtime.wasm'

NODES = 4
WORKDIR = '.'
PHRASES = ['//Alice', '//Bob', '//Cedric', '//Dick']


def check_if_files_are_built():
    assert os.path.isfile(BINARY), 'Binary is not ready'
    assert os.path.isfile(CORRUPTED_RUNTIME), 'Corrupted runtime is not ready'
    assert os.path.isfile(FIXING_RUNTIME), 'Fixing runtime is not ready'
    assert os.path.isfile(NEW_RUNTIME), 'New runtime is not ready'


def update_to_corrupted():
    print('Updating to the corrupted runtime')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], CORRUPTED_RUNTIME])
    sleep(2)


def check_update_possibility(chain):
    print('Updating to the new runtime')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], NEW_RUNTIME])
    sleep(2)
    query_runtime_version(chain)


def test_code_substitute():
    check_if_files_are_built()

    chain = run_binary(WORKDIR, BINARY, PHRASES, 'old')
    query_runtime_version(chain)
    check_highest(chain)

    update_to_corrupted()
    query_runtime_version(chain)
    check_highest(chain)

    stalled_hash, finalized = wait_for_stalling(chain)

    update_chainspec(stalled_hash, FIXING_RUNTIME)
    restart_nodes(chain, 'chainspec-new.json')

    wait_for_continuation(chain, finalized)

    check_update_possibility(chain)
    check_highest(chain)
    stop(chain)


if __name__ == '__main__':
    test_code_substitute()

#!/bin/env python
import os
import os.path
import subprocess
from time import sleep

from code_substitute_utils import query_runtime_version, restart_nodes, stop, \
    check_highest, update_chainspec, run_binary

SEND_RUNTIME = 'send-runtime/target/release/send_runtime'

BINARY = 'test-code-substitute/build/aleph-node'
ON_CHAIN_RUNTIME = 'test-code-substitute/build/on_chain_runtime.wasm'
CORRUPTED_RUNTIME = 'test-code-substitute/build/corrupted_runtime.wasm'
SHADOWING_RUNTIME = 'test-code-substitute/build/shadowing_runtime.wasm'

NODES = 4
WORKDIR = '.'
PHRASES = ['//Alice', '//Bob', '//Cedric', '//Dick']


def check_if_files_are_built():
    assert os.path.isfile(BINARY), 'Binary is not ready'
    assert os.path.isfile(ON_CHAIN_RUNTIME), 'On-chain runtime is not ready'
    assert os.path.isfile(CORRUPTED_RUNTIME), 'Corrupted runtime is not ready'
    assert os.path.isfile(SHADOWING_RUNTIME), 'Shadowing runtime is not ready'


def update_to_on_chain():
    print('Updating runtime to use on-chain blob')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], ON_CHAIN_RUNTIME])
    sleep(2)


def update_to_corrupted():
    print('Trying to update runtime to use corrupted version')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], CORRUPTED_RUNTIME])
    sleep(2)


def test_code_substitute():
    check_if_files_are_built()

    chain = run_binary(WORKDIR, BINARY, PHRASES, 'old')
    query_runtime_version(chain)

    block_hash = chain[0].get_hash(check_highest(chain))
    update_chainspec(block_hash, SHADOWING_RUNTIME)
    restart_nodes(chain, 'chainspec-new.json')
    sleep(10)

    update_to_on_chain()
    query_runtime_version(chain)
    check_highest(chain)

    update_to_corrupted()
    query_runtime_version(chain)
    check_highest(chain)
    sleep(5)

    stop(chain)


if __name__ == '__main__':
    test_code_substitute()

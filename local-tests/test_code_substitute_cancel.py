#!/bin/env python
import os
import os.path
import subprocess

from code_substitute_utils import *

SEND_RUNTIME = 'send-runtime/target/release/send_runtime'

BINARY = 'test-code-substitute/build/aleph-node'
ON_CHAIN_RUNTIME = 'test-code-substitute/build/on_chain_runtime.wasm'
TEMPORARY_RUNTIME = 'test-code-substitute/build/temporary_runtime.wasm'

NODES = 4
WORKDIR = '.'
PHRASES = ['//Alice', '//Bob', '//Cedric', '//Dick']

EXTRINSIC = '0x2d02840030f8911d2b4f40d22c3b71c26cd3f49dd938a7f91bbd62bc732fe921f19003560156abd9ef33ce4f057cdaf9cba5d' \
            '83320b3ff48b7dd79e48fe3c3fa570d867561cdbc9a52b955685a01161ee96263bca4ceddbd1907e99a2fbbda05e144fd818a00' \
            '040004000098c2366f07c9631d23a57fba9e8624541d52f25a61dea9cfed20ea792640bd00a10f'


def check_if_files_are_built():
    assert os.path.isfile(BINARY), 'Binary is not ready'
    assert os.path.isfile(ON_CHAIN_RUNTIME), 'On-chain runtime is not ready'
    assert os.path.isfile(TEMPORARY_RUNTIME), 'Temporary runtime is not ready'


def update_to_on_chain():
    print('Updating runtime to use on-chain blob')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], ON_CHAIN_RUNTIME])
    sleep(2)


def print_fees_now(chain):
    print(f'Querying fee details of an extrinsic at current block')
    print(chain[0].rpc('payment_queryFeeDetails', [EXTRINSIC]))


def test_code_substitute():
    check_if_files_are_built()

    chain = run_binary(WORKDIR, BINARY, PHRASES, 'old')
    query_runtime_version(chain)
    check_highest(chain)

    update_to_on_chain()
    query_runtime_version(chain)
    check_highest(chain)

    sleep(5)
    print_fees_now(chain)

    block_hash = chain[0].get_hash(check_highest(chain))
    update_chainspec(block_hash, TEMPORARY_RUNTIME)
    restart_nodes(chain, 'chainspec-new.json')
    sleep(10)
    check_highest(chain)
    print_fees_now(chain)

    restart_nodes(chain, 'chainspec.json', 'previous')
    sleep(10)
    check_highest(chain)
    print_fees_now(chain)

    stop(chain)


if __name__ == '__main__':
    test_code_substitute()

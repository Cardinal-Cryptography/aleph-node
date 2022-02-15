#!/bin/env python
import os
import os.path
import subprocess

from time import sleep

from code_substitute_utils import run_binary, query_runtime_version, \
    check_highest, update_chainspec, stop, restart_nodes

SEND_RUNTIME = 'send-runtime/target/release/send_runtime'

BINARY = 'test-code-substitute/build/aleph-node'
ON_CHAIN_RUNTIME = 'test-code-substitute/build/on_chain_runtime.wasm'
LYING_RUNTIME = 'test-code-substitute/build/lying_runtime.wasm'

NODES = 4
WORKDIR = '.'
PHRASES = ['//Alice', '//Bob', '//Cedric', '//Dick']

EXTRINSIC = '0x2d02840030f8911d2b4f40d22c3b71c26cd3f49dd938a7f91bbd62bc732fe9' \
            '21f19003560156abd9ef33ce4f057cdaf9cba5d83320b3ff48b7dd79e48fe3c3' \
            'fa570d867561cdbc9a52b955685a01161ee96263bca4ceddbd1907e99a2fbbda' \
            '05e144fd818a00040004000098c2366f07c9631d23a57fba9e8624541d52f25a' \
            '61dea9cfed20ea792640bd00a10f'


def check_if_files_are_built():
    assert os.path.isfile(BINARY), 'Binary is not ready'
    assert os.path.isfile(ON_CHAIN_RUNTIME), 'On-chain runtime is not ready'
    assert os.path.isfile(LYING_RUNTIME), 'Lying runtime is not ready'


def update_to_on_chain():
    print('Updating runtime to use on-chain blob')
    subprocess.check_call(
        [SEND_RUNTIME, '--url', 'localhost:9945', '--sudo-phrase', PHRASES[0], ON_CHAIN_RUNTIME])
    sleep(2)


def print_fees_at(chain, block_hash):
    print(f'Querying fee details of an extrinsic at hash {block_hash}')
    print(chain[0].rpc('payment_queryInfo', [EXTRINSIC, block_hash]))


def test_code_substitute():
    check_if_files_are_built()

    chain = run_binary(WORKDIR, BINARY, PHRASES, 'old')
    query_runtime_version(chain)
    check_highest(chain)

    update_to_on_chain()
    query_runtime_version(chain)
    sleep(10)

    block_num = check_highest(chain)
    block_hash = chain[0].get_hash(block_num)
    prev_block_hash = chain[0].get_hash(block_num - 1)

    print_fees_at(chain, block_hash)
    sleep(5)

    update_chainspec(prev_block_hash, LYING_RUNTIME)
    restart_nodes(chain, 'chainspec-new.json')
    sleep(10)
    check_highest(chain)

    print_fees_at(chain, block_hash)
    sleep(5)
    stop(chain)


if __name__ == '__main__':
    test_code_substitute()

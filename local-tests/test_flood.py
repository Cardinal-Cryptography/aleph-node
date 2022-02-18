#!/bin/env python
import os
import subprocess
import sys
from os.path import abspath, join
from time import sleep, time

from chainrunner import Chain, Seq, generate_keys

# Path to working directory, where chainspec, logs and nodes' dbs are written:
workdir = abspath(os.getenv('WORKDIR', '/tmp/workdir'))
# Path to the aleph-node binary:
binary = abspath(os.getenv('BINARY', join(workdir, 'aleph-node')))
# Path to the flooder script
flooder = abspath(os.getenv('FLOODER', join(workdir, '../.github/scripts/flooder.sh')))


def check_highest(nodes):
    results = [node.highest_block() for node in nodes]
    highest, finalized = zip(*results)
    print('Blocks seen by nodes:')
    print('  Highest:   ', *highest)
    print('  Finalized: ', *finalized)

    return max(highest)


phrases = ['//Alice', '//Bob', '//Charlie', '//Dave']
keys = generate_keys(binary, phrases)
chain = Chain(workdir)
MILLISECS_PER_BLOCK = 1000
print('Bootstraping the chain...')
chain.bootstrap(binary,
                keys.values(),
                sudo_account_id=keys[phrases[0]],
                chain_type='local',
                millisecs_per_block=MILLISECS_PER_BLOCK,
                session_period=40)

chain.set_flags('validator',
                port=Seq(30334),
                ws_port=Seq(9944),
                rpc_port=Seq(9933),
                unit_creation_delay=200,
                execution='Native')

chain.start('aleph')

print('Waiting a bit')
sleep(30)

start = time()
old = check_highest(chain)

subprocess.run([flooder], check=True).check_returncode()
new = check_highest(chain)

expected_blocks = int((time() - start) * 1000) // MILLISECS_PER_BLOCK
EPSILON = 20

if new - old + EPSILON < expected_blocks:
    print(f'Expected at least {expected_blocks - EPSILON} new blocks, got {new - old}')
    sys.exit(1)

#!/bin/env python
import os
import sys
from os.path import abspath, join
from time import sleep

from chainrunner import Chain, Seq, generate_keys

# Path to working directory, where chainspec, logs and nodes' dbs are written:
workdir = abspath(os.getenv('WORKDIR', '/tmp/workdir'))
# Path to the aleph-node binary (important use short-session feature):
binary = abspath(os.getenv('BINARY', join(workdir, 'aleph-node')))



def check_finalized(nodes):
    results = [node.highest_block() for node in nodes]
    highest, finalized = zip(*results)
    print('Blocks seen by nodes:')
    print('  Highest:   ', *highest)
    print('  Finalized: ', *finalized)

    return finalized


phrases = [f'//{i}' for i in range(6)]
keys = generate_keys(binary, phrases)
all_accounts = list(keys.values())
chain = Chain(workdir)
print('Bootstraping the chain with old binary')
chain.bootstrap(binary,
                all_accounts[:4],
                accounts=all_accounts[4:],
                sudo_account_id=keys[phrases[0]],
                chain_type='local')

chain.set_flags(port=Seq(30334),
                ws_port=Seq(9944),
                rpc_port=Seq(9933),
                unit_creation_delay=200,
                execution='Native')

chain.set_flags('validator', predicate=lambda n, i: i < 4)

print('Starting the chain')
chain.start('aleph')

print('Waiting 30s')
sleep(30)

check_finalized(chain)
print('Killing one validator and one nonvalidator')
chain[3].stop()
chain[4].stop()

print('waiting around 4 sessions')
sleep(30 * 4)

print('restarting nodes')
chain.start('aleph', nodes=[3, 4])
check_finalized(chain)
sleep(30)
finalized_per_node = check_finalized(chain)

nonvalidator_diff = finalized_per_node[5] - finalized_per_node[4]
validator_diff = finalized_per_node[2] - finalized_per_node[3]
ALLOWED_DELTA = 5

if nonvalidator_diff > ALLOWED_DELTA:
    print(f"too big difference for nonvalidators: {nonvalidator_diff}")
    sys.exit(1)

if validator_diff > ALLOWED_DELTA:
    print(f"too big difference for nonvalidators: {validator_diff}")
    sys.exit(1)

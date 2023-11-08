#!/bin/env python

import os
from os.path import abspath, join
import logging
from chainrunner import Chain, Seq, generate_keys, check_finalized

logging.basicConfig(format='%(asctime)s %(message)s')
workdir = abspath(os.getenv('WORKDIR', '/tmp/workdir'))
binary = abspath(os.getenv('ALEPH_NODE_BINARY', join(workdir, 'aleph-node')))

phrases = [f'//{i}' for i in range(5)]
keys = generate_keys(binary, phrases)
all_accounts = list(keys.values())
chain = Chain(workdir)

chain.new(binary, all_accounts)

chain.set_flags('no-mdns',
                port=Seq(30334),
                validator_port=Seq(30343),
                rpc_port=Seq(9944),
                unit_creation_delay=200,
                execution='Native')
addresses = [n.address() for n in chain]
validator_addresses = [n.validator_address() for n in chain]
chain.set_flags(bootnodes=addresses[0])
chain.set_flags_validator(public_addr=addresses,
                          public_validator_addresses=validator_addresses)

chain.set_flags_validator('validator')

logging.info('Starting the chain')
chain.start('aleph', nodes=[0, 1, 2, 3])
check_finalized(chain)
logging.info('Waiting for 2700 blocks to finalize (3 sessions)')
chain.wait_for_finalization(old_finalized=0,
                            finalized_delta=2700,
                            catchup=True,
                            catchup_delta=5,
                            timeout=60 * 60)
check_finalized(chain)
logging.info('Starting 4th node')
chain.start('aleph', nodes=[4])
logging.info('Waiting for 4th node to catch up')
chain.wait_for_finalization(old_finalized=0,
                            nodes=[4],
                            finalized_delta=2700,
                            catchup=True,
                            catchup_delta=5,
                            timeout=10 * 60)
check_finalized(chain)
logging.info('OK')

#!/bin/env python

# Short script demonstrating the basic usage of `chainrunner` package.
# Reproduces (more or less) the behavior of `run_nodes.sh`.
# For running local experiments it's much more convenient to manage the chain
# using an interactive environment (Python console, Jupyter notebook etc.)

from chainrunner import Chain, Seq, generate_keys, check_finalized

NODES = 4
WORKDIR = '.'
BINARY = '../target/release/aleph-node'
PORT = 30334
WS_PORT = 9944
RPC_PORT = 9933

PHRASES = ['//Alice', '//Bob', '//Charlie', '//Dave', '//Ezekiel', '//Fanny', '//George', '//Hugo']
keys_dict = generate_keys(BINARY, PHRASES)
keys = list(keys_dict.values())
nodes = min(NODES, len(PHRASES))

chain = Chain(WORKDIR)

print(f'Bootstrapping chain for {nodes} nodes')
chain.bootstrap(BINARY,
                keys[:nodes],
                chain_type='local')
chain.set_flags('validator',
                'unsafe-ws-external',
                'unsafe-rpc-external',
                'no-mdns',
                port=Seq(PORT),
                ws_port=Seq(WS_PORT),
                rpc_port=Seq(RPC_PORT),
                unit_creation_delay=500,
                execution='Native',
                rpc_cors='all',
                rpc_methods='Unsafe',
                pruning='archive')
addresses = [n.address() for n in chain]
chain.set_flags(bootnodes=addresses[0], public_addr=addresses)

print('Starting the chain')
chain.start('node')

print('Waiting for finalization')
chain.wait_for_finalization(0)

check_finalized(chain)

print('Exiting script, leaving nodes running in the background')

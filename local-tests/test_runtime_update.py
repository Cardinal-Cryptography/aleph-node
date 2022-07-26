#!/bin/env python
import argparse
import json
import logging
import os
import subprocess
from pathlib import Path

from chainrunner import Chain, Seq, generate_keys

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s %(levelname)-8s %(message)s',
)

WORKDIR = os.path.abspath(os.getenv('WORKDIR', '/tmp/workdir'))


def file(filepath: str) -> Path:
    logging.debug(f'Looking for file {filepath}...')
    path = Path(filepath)
    if not path.is_file():
        raise argparse.ArgumentTypeError(f'❌ File `{filepath}` was not found')
    return path


def get_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description='Test runtime update with `try-runtime`')

    parser.add_argument('old_binary', type=file, help='Path to the old binary (live chain version)')
    parser.add_argument('new_runtime', type=file, help='Path to the new runtime')
    parser.add_argument('try_runtime', type=file, help='Path to the `try-runtime` tool')

    parser.add_argument('--wait_for', type=int, default=1,
                        help='Wait until this many blocks are finalized before trying update. '
                             'By default `1`')

    return parser.parse_args()


def save_runtime_to_chainspec(chainspec_path: Path, runtime_path: Path):
    logging.info(f'Setting `code` in {chainspec_path} to the content of {runtime_path}...')

    with open(chainspec_path, mode='r', encoding='utf-8') as chainspec_file:
        chainspec = json.loads(chainspec_file.read())
    logging.debug(f'✅ Read chainspec from {chainspec_path}')

    with open(runtime_path, mode='rb') as runtime_file:
        runtime = runtime_file.read().hex()
    logging.debug(f'✅ Read runtime from {runtime_path}')

    chainspec['genesis']['runtime']['system']['code'] = f'0x{runtime}'

    with open(chainspec_path, mode='w', encoding='utf-8') as chainspec_file:
        chainspec_file.write(json.dumps(chainspec, indent=2))
    logging.info(f'✅ Saved updated chainspec to {chainspec_path}')


def start_chain(binary: Path, wait_for: int) -> Chain:
    logging.info(f'Starting live chain using {binary}...')

    phrases = [f'//{i}' for i in range(6)]
    keys = generate_keys(binary, phrases)
    all_accounts = list(keys.values())

    chain = Chain(WORKDIR)

    logging.debug(f'Bootstrapping the chain with binary {binary}...')
    chain.bootstrap(binary,
                    all_accounts[:4],
                    nonvalidators=all_accounts[4:],
                    sudo_account_id=keys[phrases[0]],
                    chain_type='local',
                    raw=False)

    chain.set_flags('no-mdns',
                    port=Seq(30334),
                    ws_port=Seq(9944),
                    rpc_port=Seq(9933),
                    unit_creation_delay=200,
                    execution='Native',
                    pruning='archive')
    addresses = [n.address() for n in chain]
    chain.set_flags(bootnodes=addresses[0], public_addr=addresses)

    chain.set_flags_validator('validator')

    chain.start('aleph')
    logging.info('Live chain started. Waiting for finalization and authorities.')

    chain.wait_for_finalization(wait_for)
    chain.wait_for_authorities()
    logging.debug('Initial checks passed, chain seems to be fine')

    return chain


def test_update(try_runtime: Path, chainspec: Path):
    cmd = [try_runtime, 'try-runtime', '--chain', chainspec, 'on-runtime-upgrade', 'live',
           '--uri', 'ws://localhost:9944']
    logging.info('Running `try-runtime` check...')
    subprocess.run(cmd, check=True)
    logging.info('✅ Update has been successful!')


def run_test(old_binary: Path, new_runtime: Path, try_runtime: Path, wait_for: int):
    chain = start_chain(old_binary, wait_for)
    chainspec = file(os.path.join(WORKDIR, 'chainspec.json'))
    save_runtime_to_chainspec(chainspec, new_runtime)
    test_update(try_runtime, chainspec)
    chain.stop()
    chain.purge()


if __name__ == '__main__':
    args = get_args()
    run_test(args.old_binary, args.new_runtime, args.try_runtime, args.wait_for)

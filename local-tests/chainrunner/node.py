import json
import jsonrpcclient
import os.path as op
import re
import requests
import subprocess
import time
from jsonrpcclient import Error, Ok
from collections import namedtuple

from substrateinterface import SubstrateInterface, Keypair

from .utils import flags_from_dict, check_file, check_finalized

NodeInfoCache = namedtuple("NodeInfoCache", ["highest_block", "highest_finalized"])

class Node:
    """A class representing a single node of a running blockchain.
    `binary` should be a path to a file with aleph-node binary.
    `chainspec` should be a path to a file with chainspec,
    `path` should point to a folder where the node's base path is."""

    def __init__(self, idx, binary, chainspec, path, logdir=None):
        self.idx = idx
        self.chainspec = chainspec
        self.binary = binary
        self.path = path
        self.logdir = logdir or path
        self.logfile = None
        self.process = None
        self.flags = {}
        self.running = False
        self.info_cache = NodeInfoCache(highest_block=-1, highest_finalized=-1)

    def _stdargs(self):
        return ['--base-path', self.path, '--chain', self.chainspec]

    def _nodeargs(self, backup=True):
        res = ['--node-key-file', op.join(self.path, 'p2p_secret'), '--enable-log-reloading']
        if backup:
            res += ['--backup-path', op.join(self.path, 'backup-stash')]
        return res

    def start(self, name, backup=True):
        """Start the node. `name` is used to name the logfile and for the --name flag."""
        if self.running:
            print('Node already running')
            return
        name = f'{name}{self.idx}'
        cmd = [self.binary, '--name', name] + self._stdargs() + self._nodeargs(backup) + flags_from_dict(self.flags)
        self.logfile = op.join(self.logdir, name + '.log')
        with open(self.logfile, 'w', encoding='utf-8') as logfile:
            self.process = subprocess.Popen(cmd, stderr=logfile, stdout=subprocess.DEVNULL)
        self.running = True

    def stop(self):
        """Stop the node by sending SIGTERM."""
        if self.running:
            self.process.terminate()
            self.running = False

    def purge(self):
        """Purge chain (delete the database of the node)."""
        cmd = [self.binary, 'purge-chain', '-y'] + self._stdargs()
        subprocess.run(cmd, stdout=subprocess.DEVNULL)

    def rpc_port(self):
        """Return RPC port for this node. The value is taken from `flags` dictionary.
        Raises KeyError if not present."""
        port = self.flags.get('rpc_port', self.flags.get('rpc-port'))
        if port is None:
            raise KeyError("RPC port unknown, please set rpc_port flag")
        return port

    def greplog(self, regexp):
        """Find in the logs all occurrences of the given regexp. Returns a list of matches."""
        if not self.logfile:
            return []
        with open(self.logfile, encoding='utf-8') as f:
            log = f.read()
        return re.findall(regexp, log)

    def highest_hash(self):
        highest_block = self.rpc('chain_getBlockHash', None)
        if isinstance(highest_block, Ok):
            return highest_block.result
        else:
            return None

    def highest_finalized_hash(self):
        highest_finalized = self.rpc('chain_getFinalizedHead', None)
        if isinstance(highest_finalized, Ok):
            return highest_finalized.result
        else:
            return None

    def block_number(self, hash):
        block = self.rpc('chain_getBlock', [hash])
        if isinstance(block, Ok):
            return int(block.result['block']['header']['number'], 16)
        else:
            return -1

    def highest_block(self):
        """Find the height of the most recent block.
        Return two ints: highest block and highest finalized block."""
        try:
            highest_block_hash = self.highest_hash()
            highest_finalized_hash = self.highest_finalized_hash()
            highest_block, highest_finalized = self.block_number(highest_block_hash), self.block_number(highest_finalized_hash)
        except Exception:
            highest_block, highest_finalized = -1, -1

        highest_block = highest_block if highest_block != -1 else self.info_cache.highest_block
        highest_finalized = highest_finalized if highest_finalized != -1 else self.info_cache.highest_finalized
        self.info_cache = NodeInfoCache(highest_block=highest_block, highest_finalized=highest_finalized)

        return highest_block, highest_finalized

    def check_authorities(self):
        """Find in the logs the number of authorities this node is connected to.
        Return bool indicating if it's connected to all known authorities."""
        grep = self.greplog(r'(\d+)/(\d+) authorities known for session')
        return grep[-1][0] == grep[-1][1] if grep else False

    def get_hash(self, height):
        """Find the hash of the block with the given height. Requires the node to be running."""
        return self.rpc('chain_getBlockHash', [height]).result

    def state(self, block=None):
        """Return a JSON representation of the chain state after the given block.
        If `block` is `None`, the most recent state (after the highest seen block) is returned.
        Node must not be running, empty result is returned if called on a running node."""
        if self.running:
            print("cannot export state of a running node")
            return {}
        cmd = [self.binary, 'export-state'] + self._stdargs()
        if block is not None:
            cmd.append(str(block))
        proc = subprocess.run(cmd, capture_output=True, check=True)
        return json.loads(proc.stdout)

    def rpc(self, method, params=None):
        """Make an RPC call to the node with the given method and params.
        `params` should be a tuple for positional arguments, or a dict for keyword arguments."""
        if not self.running:
            print("cannot RPC because node is not running")
            return None
        port = self.rpc_port()
        resp = requests.post(f'http://localhost:{port}/', json=jsonrpcclient.request(method, params))
        return jsonrpcclient.parse(resp.json())

    def set_log_level(self, target, level):
        """Change log verbosity of the chosen target.
        This method should be called on a running node."""
        return self.rpc('system_addLogFilter', [f'{target}={level}'])

    def address(self, port=None):
        """Get the public address of this node. Returned value is of the form
        /dns4/localhost/tcp/{PORT}/p2p/{KEY}. This method needs to know node's port -
        if it's not supplied a as parameter, it must be present in `self.flags`.
        """
        if port is None:
            if 'port' in self.flags:
                port = self.flags['port']
            else:
                return None
        cmd = [self.binary, 'key', 'inspect-node-key', '--file', op.join(self.path, 'p2p_secret')]
        output = subprocess.check_output(cmd).decode().strip()
        return f'/dns4/localhost/tcp/{port}/p2p/{output}'

    def validator_address(self, port=None):
        """Get the public validator address of this node. Returned value is of the form
        localhost:{PORT}. This method needs to know node's validator port -
        if it's not supplied a as parameter, it must be present in `self.flags`.
        """
        if port is None:
            if 'validator_port' in self.flags:
                port = self.flags['validator_port']
            else:
                return None
        return f'localhost:{port}'

    def change_binary(self, new_binary, name, purge=False):
        """Stop the node and change its binary to `new_binary`.
        Optionally `purge` node's database.
        Restart the node with new `name`.
        Returns the highest finalized block seen by node before shutdown."""
        new_binary = check_file(new_binary)
        self.stop()
        time.sleep(5)
        highest_finalized = check_finalized([self])[0]
        if purge:
            self.purge()
        self.binary = new_binary
        self.start(name)
        return highest_finalized

    def update_finality_version(self, session, sudo_phrase):
        """Bump the finality version stored on chain by 1."""
        port = self.rpc_port()
        subint = SubstrateInterface(url=f'ws://localhost:{port}', ss58_format=42)
        version = subint.query(module='Aleph', storage_function='FinalityVersion').value
        set_version_call = subint.compose_call(call_module='Aleph', call_function='schedule_finality_version_change', call_params={'version_incoming': version + 1, 'session': session})
        zero_weight = {"proof_size": 0, "ref_time": 0}
        sudo_call = subint.compose_call(call_module='Sudo', call_function='sudo_unchecked_weight', call_params={'call': set_version_call, 'weight': zero_weight})
        extrinsic = subint.create_signed_extrinsic(call=sudo_call, keypair=Keypair.create_from_uri(sudo_phrase))
        return subint.submit_extrinsic(extrinsic, wait_for_inclusion=True)

    def update_runtime(self, runtime_path, sudo_phrase):
        """Compose and submit `set_code` extrinsic containing runtime from supplied `runtime_path`.
        `sudo_phrase` should be the seed phrase for chain's sudo account.
        Returns an instance of ExtrinsicReceipt."""
        with open(check_file(runtime_path), 'rb') as file:
            runtime = file.read()
        port = self.rpc_port()
        subint = SubstrateInterface(url=f'ws://localhost:{port}', ss58_format=42)
        set_code_call = subint.compose_call(call_module='System', call_function='set_code', call_params={'code': runtime})
        zero_weight = {"proof_size":0, "ref_time":0}
        sudo_call = subint.compose_call(call_module='Sudo', call_function='sudo_unchecked_weight', call_params={'call': set_code_call, 'weight':zero_weight})
        extrinsic = subint.create_signed_extrinsic(call=sudo_call, keypair=Keypair.create_from_uri(sudo_phrase))
        receipt = subint.submit_extrinsic(extrinsic, wait_for_inclusion=True)
        return receipt

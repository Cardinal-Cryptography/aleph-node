import random
import subprocess
import json
from tabulate import tabulate
import urllib.request

AZERO = 1_000_000_000_000


def event_field(event, field):
    for f in event['fields']:
        if f['name'] == field:
            return f['value']


def account_id(value):
    match value:
        case {'Literal': account_id}: return account_id
        case _: raise ValueError(f'Invalid account id: {value}')


def uint(value):
    match value:
        case {'UInt': value}: return value
        case _: raise ValueError(f'Invalid uint: {value}')


def deployer_account_id(deploy_result):
    address = deploy_result['contract']
    setup_event = next(filter(
        lambda e: e['name'] == 'Transfer' and account_id(event_field(e, 'to')) == address, deploy_result['events']), None)

    return account_id(event_field(setup_event, 'from'))


def find_fee(events, by_whom):
    fee_event = next(filter(lambda e: e['name'] == 'TransactionFeePaid' and account_id(
        event_field(e, 'who')) == by_whom, events), None)
    return uint(event_field(fee_event, 'actual_fee'))


def random_salt():
    return ''.join(random.choice('0123456789abcdef') for _ in range(10))


def price_storage_deposit(call, directory):
    res = subprocess.check_output(call + ["--dry-run"], cwd=directory)
    res = json.loads(res.decode('utf-8'))
    return res['storage_deposit']['Charge']


class Pricing:
    def __init__(self, suri, url):
        self.suri = suri
        self.url = url
        self.prices = {}
        self.addresses = {}
        self.directories = {}
        self.suri_address = None
        self.storage_deposits = {}

        with urllib.request.urlopen('https://api.coingecko.com/api/v3/simple/price?ids=aleph-zero&vs_currencies=usd') as response:
            data = json.load(response)
            self.aleph_usd = data['aleph-zero']['usd']

    def instantiate(self, directory, alias, args=[]):
        """Instantiate a contract from a given directory.

        The contract will be referred to by the given alias in subsequent calls.
        The price and storage fee of the instantiation will be recorded.

        Keyword arguments:
        args -- arguments to pass to the contract constructor (default [])
        """
        call_args = [x for a in args for x in ['--args', a]]
        call = ['cargo', 'contract', 'instantiate', '--salt',
                random_salt()] + call_args + self.common_args()
        action = "Instantiate %s" % alias

        self.storage_deposits[action] = price_storage_deposit(call, directory)
        res = subprocess.check_output(call, cwd=directory)
        res = json.loads(res.decode('utf-8'))

        self.register(res['contract'], alias, directory)
        self.suri_address = deployer_account_id(res)
        self.prices[action] = find_fee(res['events'], self.suri_address)

    def register(self, address, alias, directory):
        """Register an already-deployed contract under the given alias.

        Note that providing a directory with the contract code is needed, because
        this module uses `cargo contract` to make the calls to the contract.
        """
        self.addresses[alias] = address
        self.directories[alias] = directory

    def call(self, alias, message, value=0, args=[], silent=False):
        """Call a contract method.

        Keyword arguments:
        value -- transfer this number of native tokens with the call (default 0)
        args -- arguments to pass to the contract method (default [])
        silent -- if True, the price and storage fee of the call will not be recorded (default False)
        """
        contract = self.addresses[alias]
        directory = self.directories[alias]
        action = "Call %s::%s" % (alias, message)
        call_args = [x for a in args for x in ['--args', a]]

        call = ['cargo', 'contract', 'call', '--contract', contract, '--value', str(value),
                '--message', message] + call_args + self.common_args()
        if not silent:
            self.storage_deposits[action] = price_storage_deposit(
                call, directory)

        res = subprocess.check_output(call, cwd=directory)
        res = json.loads(res.decode('utf-8'))

        if not silent:
            self.prices[action] = find_fee(res, self.suri_address)

    def table(self):
        """Return a table with the recorded prices and storage fees as a string."""
        headers = ['Operation', 'Execution Fee', 'Storage Deposit']
        rows = [[
            k, self.format_fee(v), self.format_fee(self.storage_deposits[k])
        ] for k, v in self.prices.items()]

        return tabulate(rows, headers=headers, tablefmt="github")

    def format_fee(self, fee):
        return "%f AZERO ($%f)" % (fee / AZERO, fee / AZERO * self.aleph_usd)

    def common_args(self):
        return ['--suri', self.suri, '--url',
                self.url, '--skip-confirm', '--output-json']

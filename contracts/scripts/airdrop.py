from csv import reader
from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.base import SubstrateRequestException
from substrateinterface.contracts import ContractCode, ContractInstance
from tqdm import tqdm
import argparse
import os

# Example usage:
# python3 contracts/scripts/airdrop.py -p $(pwd)/contracts/scripts/stakers_mainnet.list -s 'bottom drive obey lake curtain smoke basket hold race lonely fit walk//Alice' -c 5Dxt8scUb5qzhhEAAyYVjq1HG4zgNKJsWxhYFChxmPhgm648 -m $PWD/contracts/ticket_token/target/ink/ticket_token.json
#

def prepare_tx(contract, to, gas_limit, storage_limit):

    return interface.compose_call(
        call_module='Contracts',
        call_function='call',
        call_params={
            'dest': contract,
            'value': 0,
            'gas_limit': gas_limit, #{'ref_time': gas_limit, 'proof_size': 16689},
            'storage_deposit_limit': storage_limit
        }
    )

def read_array(path, batch_size):
    with open(path, 'r') as f:
        raw_data = list(reader(f))
        for i in range(0, len(raw_data), batch_size):
            yield raw_data[i: i+batch_size]

if __name__ == '__main__':

    parser = argparse.ArgumentParser()
    parser.add_argument('-p', '--players', help = 'path to file with list of player addresses', required = True)
    parser.add_argument('-n', '--node', help = 'node ws endpoint', default = 'ws://127.0.0.1:9944')
    parser.add_argument('-s', '--seed_phrase', help='secret seed of the private key to sign txs with', required = True)
    parser.add_argument('-b', '--batch_size', help = 'batch size', default = 100, type = int)
    parser.add_argument('-c', '--contract', help = 'on-chain contract address', required = True)
    parser.add_argument('-m', '--metadata', help = 'contract metadata (path)', required = True)
    args = parser.parse_args()

    print('Executing airdrop:', args)

    players = read_array(args.players, args.batch_size)
    players = list(players)
    print('Loaded players list:', players)

    node = args.node
    interface = SubstrateInterface(url = node, ss58_format = 42)
    print("Connected to " + node)

    keypair = Keypair.create_from_uri(args.seed_phrase)
    print(f"Using account {keypair.ss58_address}")

    contract_info = interface.query("Contracts", "ContractInfoOf", [args.contract])
    if contract_info.value:
        print(f'Found contract on chain: {contract_info.value}')
        # Create contract instance
        contract = ContractInstance.create_from_address(
          contract_address = args.contract,
          metadata_file = os.path.join(args.metadata),
          substrate = interface
        )
    else:
        raise Exception("Contract not on chain")

    dry_run_result = contract.read(keypair, 'PSP22::transfer', args = {"to": players[0][0][0], "value" : int(3), "data" : []})
    print('Dry run result:', dry_run_result)

    gas_limit = dry_run_result.gas_required
    # storage_limit = dry_run_result['storage_deposit'][1]
    # print('storage_limit', storage_limit)
    
    methods = [method_name for method_name in dir(contract)
                  if callable(getattr(contract, method_name))]
    print("@ contract", methods)

    # we send 3 tickets
    amount = 3
    for batch in tqdm(players):
        recipients = list(zip(*batch))[0]

        calls = [prepare_tx(args.contract, to[0], gas_limit, 1920000000) for to in batch]

    print('Done.')

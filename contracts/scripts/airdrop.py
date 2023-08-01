from csv import reader
from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.base import SubstrateRequestException
from substrateinterface.contracts import ContractCode, ContractInstance, ContractMetadata
from tqdm import tqdm
import argparse
import os

# Will send 3 units of a PSP22 token to each user in the list
#
# Example:
#
# python3 contracts/scripts/airdrop.py -p $(pwd)/contracts/scripts/stakers_mainnet.list -s 'bottom drive obey lake curtain smoke basket hold race lonely fit walk//Alice' -c 5Dxt8scUb5qzhhEAAyYVjq1HG4zgNKJsWxhYFChxmPhgm648 -m $PWD/contracts/ticket_token/target/ink/ticket_token.json
#
# Adapted from:
# https://github.com/polkascan/py-substrate-interface/blob/3826ebe325b12a5c942f8cb3954480bee5ae7ca5/substrateinterface/contracts.py#L816-L834

def prepare_tx(metadata, address, to, gas_limit, storage_limit):
    # encode contract call data
    # we send 3 tickets to each player
    call = metadata.generate_message_data('PSP22::transfer', {"to": to, "value" : int(3), "data" : []})

    return interface.compose_call(
        call_module='Contracts',
        call_function='call',
        call_params={
            'dest': address,
            'value': 0,
            'gas_limit': gas_limit,
            'storage_deposit_limit': storage_limit,
            'data': call.to_hex()
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
    parser.add_argument('-b', '--batch_size', help = 'batch size', default = 50, type = int)
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

    metadata = ContractMetadata.create_from_file(args.metadata, interface)

    processed_recipients = []
    for batch in tqdm(players):
        recipients = list(zip(*batch))[0]

        calls = [prepare_tx(metadata, args.contract, to[0], gas_limit, 1920000000) for to in batch]

        call = interface.compose_call(
            call_module = 'Utility',
            call_function = 'batch',
            call_params = {'calls': calls}
        )

        xt = interface.create_signed_extrinsic(call = call, keypair = keypair)
        try:
            receipt = interface.submit_extrinsic(xt, wait_for_inclusion = True)
            if not receipt.is_success:
                print(f'Failed to submit xt with current batch {receipt.error_message}')
                with open('processed_recipients', 'w') as f:
                    f.write(str(processed_recipients))
                exit(1)
            processed_recipients.extend(recipients)

        except SubstrateRequestException as e:
            print("Failed to send: {}".format(e))

    print('Done.')

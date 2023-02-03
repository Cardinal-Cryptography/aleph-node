#!/usr/bin/python3

import argparse
import json
from pricing import Pricing


def find_access_control():
    try:
        with open('../../contracts/addresses.json', 'r') as f:
            return json.load(f)['access_control']
    except:
        return None


parser = argparse.ArgumentParser(
    description='Check the prices of some common contract operations')
parser.add_argument('--url', type=str,
                    default='ws://localhost:9944', help='URL of the node to connect to')
parser.add_argument('--suri', type=str, default='//Alice',
                    help='Secret key URI to use for calls')
parser.add_argument('--adder-dir', type=str,
                    help='Directory of the adder contract', default='../../contracts/adder')
parser.add_argument('--wrapped-azero-dir', type=str, help='Directory of the wrapped azero contract',
                    default='../../contracts/wrapped_azero')
parser.add_argument('--game-token-dir', type=str,
                    help='Directory of the game token (representing PSP22) contract', default='../../contracts/game_token')
parser.add_argument('--access-control', type=str,
                    help='Address of the access control contract', default=find_access_control())
parser.add_argument('--access-control-dir', type=str,
                    help='Directory of the access control contract', default='../../contracts/access_control')
parser.add_argument('--simple-dex-dir', type=str,
                    help='Directory of the simple dex contract', default='../../contracts/simple_dex')
args = parser.parse_args()

pricing = Pricing(args.suri, args.url)

pricing.instantiate(args.adder_dir, 'adder')
pricing.call('adder', 'add', args=['42'])

# Requires initializer privileges on 'wrapped_azero' - see the README.
pricing.instantiate(args.wrapped_azero_dir, 'wrapped_azero')
pricing.call('wrapped_azero', 'wrap', value=100)
pricing.call("wrapped_azero", 'unwrap', args=['10'])

# Requires initializer privileges on 'game_token' - see the README.
pricing.instantiate(args.game_token_dir, 'PSP22',
                    args=['"Example token"', '"EXT"'])

pricing.register(args.access_control, 'access_control',
                 args.access_control_dir)
pricing.call('access_control', 'grant_role', args=[
             pricing.suri_address, 'Minter(%s)' % pricing.addresses['PSP22']])
pricing.call('access_control', 'grant_role', args=[
             pricing.suri_address, 'Burner(%s)' % pricing.addresses['PSP22']])

pricing.call('PSP22', 'PSP22Mintable::mint',
             args=[pricing.suri_address, '100'])
pricing.call('PSP22', 'PSP22::transfer', args=[
             pricing.addresses['adder'], '10', '[]'])
pricing.call('PSP22', 'PSP22Burnable::burn', args=[
             pricing.suri_address, '10'])

pricing.instantiate(args.simple_dex_dir, 'DEX')
pricing.call('access_control', 'grant_role', args=[
             pricing.suri_address, 'LiquidityProvider(%s)' % pricing.addresses['DEX']])
pricing.call('access_control', 'grant_role', args=[
             pricing.suri_address, 'Admin(%s)' % pricing.addresses['DEX']])

pricing.call('DEX', 'add_swap_pair', args=[
             pricing.addresses['wrapped_azero'], pricing.addresses['PSP22']])

pricing.call('wrapped_azero', 'PSP22::approve', args=[
    pricing.addresses['DEX'], '20'], silent=True)
pricing.call('PSP22', 'PSP22::approve', args=[
    pricing.addresses['DEX'], '10'])

pricing.call('DEX', 'deposit', args=["[(%s, 10), (%s, 10)]" % (
    pricing.addresses['wrapped_azero'], pricing.addresses['PSP22'])])
pricing.call('DEX', 'swap', args=[
             pricing.addresses['wrapped_azero'], pricing.addresses['PSP22'], '5', '1'])

pricing.print_table()

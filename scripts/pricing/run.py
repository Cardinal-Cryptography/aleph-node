#!/usr/bin/python3

import argparse
from pricing import Pricing

parser = argparse.ArgumentParser(
    description='Check the prices of some common contract operations')
parser.add_argument('--url', type=str,
                    default='ws://localhost:9944', help='URL of the node to connect to')
parser.add_argument('--suri', type=str, default='//Alice',
                    help='Secret key URI to use for calls')
parser.add_argument('--adder-dir', type=str,
                    help='Directory of the adder contract', default='../../contracts/adder')
args = parser.parse_args()


pricing = Pricing(args.suri, args.url)

pricing.instantiate(args.adder_dir, 'adder')
pricing.call('adder', 'add', args=['42'])

pricing.print_table()

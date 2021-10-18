import logging
import os
import re
import shutil
from argparse import Namespace
from pathlib import Path

from shell import terminate_instances_in_region
from utils import default_region


def stop_protocol(tag: str):
    logging.info('Stopping instances...')
    terminate_instances_in_region(default_region(), tag)
    logging.info('Instances stopped.')


def remove_files():
    Path('addresses').unlink(missing_ok=True)
    Path('aleph-node.zip').unlink(missing_ok=True)
    Path('chainspec.json').unlink(missing_ok=True)
    Path('libp2p_public_keys').unlink(missing_ok=True)
    Path('validator_accounts').unlink(missing_ok=True)
    Path('validator_phrases').unlink(missing_ok=True)
    Path('x').unlink(missing_ok=True)
    shutil.rmtree('accounts', ignore_errors=True)
    shutil.rmtree('bin', ignore_errors=True)
    shutil.rmtree('data', ignore_errors=True)

    for item in os.listdir(os.curdir):
        if re.match(r'data\d+\.zip', item):
            os.remove(item)


def stop_monitoring():
    os.system('docker-compose down')
    Path('prometheus.yml').unlink(missing_ok=True)


def clean(args: Namespace):
    stop_protocol(args.tag)
    remove_files()
    if args.kill_monitoring:
        stop_monitoring()

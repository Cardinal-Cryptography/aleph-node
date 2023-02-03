Pricing script
==============

The `./run.py` script in this directory will deploy some contracts and print a summary of how much some basic operations
on them cost.

It requires `python3` and an Ink 4-compatible version of `cargo contract`, to install:

```bash
$ cargo install cargo-contract --version 2.0.0-beta.1
```

It also assumes that Initializer privileges are already granted for the authority used on `wrapped_azero` and
`game_token`. You can easily set this up locally with:

```bash
pushd ../../
source contracts/env/dev
contracts/scripts/deploy.sh
popd
```

Afterwards, install the python deps and run the script:

```bash
$ pip install -r requirements.txt
$ ./run.py
```

For more info on options see:

```bash
$ ./run.py --help
```

Development
===========

The script is divided into two files: `run.py`, that describes the calls that are to be made and prices of which will be
checked, and a utility file `pricing.py` that contains a wrapper around `cargo contract` that tracks prices of calls 
made and can print the result as a table. If desired, you can write a different `run.py` that would price your own set
of calls using `pricing.py` to make that more convenient.

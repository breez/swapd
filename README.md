# Swap server

## Introduction
This project contains a swap server. In a nutshell a user requests a swap, pays
to the swap address and requests payout from the server. The server will then
pay the user over lightning, obtaining the preimage. Only with that preimage can
the server redeem the utxo onchain. If the payment is not made, the user can
refund the utxo to itself once the locktime has expired.

## Installation
In order to install `swapd` you have to compile from source. 

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) installation
- Core lightning >= **v24.08**.

### Compilation
Run `make release` in the root folder of this repository. The binaries will be
`target/release/swapd` and `target/release/swap-cli`.

## Running
swapd needs cln, bitcoind and postgres to run. See `swapd --help` for
configuration parameters.

## Testing
To run all tests call `make test`, or `PYTEST_PAR=10 make test -j12`.

### Unit tests
To run only the unit tests call `make utest`.

### Integration tests

#### Prerequisites
In order to run the integration tests you need 
- python >= 3.8, < 4.0
- `virtualenv` (`python -m pip install --user virtualenv`)
- `lightningd` accessible through your `PATH`
- `bitcoind` and `bitcoin-cli` accessible through your `PATH`

#### Running integration tests
Call `make itest` to only run the integration tests. To run a single test, use
`PYTEST_OPTS="-k name_of_test" make itest`. To run tests in parallel, use
`PYTEST_PAR=10 make itest`.

## Contributing
Contributions are welcome!
Make sure to run `make check` before committing, or 
`PYTEST_PAR=10 make check -j12` if you like to be fast.
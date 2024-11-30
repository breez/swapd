from binascii import hexlify
from bitcoin.wallet import CBitcoinSecret
from fixtures import *
from pyln.testing.fixtures import (
    directory,
    setup_logging,
    teardown_checks,
    test_base_dir,
    test_name,
)
from pyln.testing.utils import wait_for, SLOW_MACHINE
import hashlib
import os

__all__ = [
    "bitcoind",
    "directory",
    "node_factory",
    "setup_logging",
    "teardown_checks",
    "test_base_dir",
    "test_name",
    "wait_for",
    "SLOW_MACHINE",
    "setup_user_and_swapper",
    "add_fund_init",
    "whatthefee",
    "postgres_factory",
    "swapd_factory",
]


def setup_user_and_swapper(node_factory, swapd_factory, swapd_opts=None):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd(options=swapd_opts)
    swapper.lightning_node.open_channel(user, 1000000)
    return user, swapper


def add_fund_init(user, swapper, amount=100_000_000):
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    secret_key = CBitcoinSecret.from_secret_bytes(os.urandom(32))
    public_key = secret_key.pub
    add_fund_resp = swapper.rpc.add_fund_init(user, public_key, h)
    payment_request = user.create_invoice(
        amount,
        description="test",
        preimage=hexlify(preimage).decode("ASCII"),
    )
    return add_fund_resp.address, payment_request, hexlify(h).decode("ASCII")

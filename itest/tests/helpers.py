from binascii import hexlify
from bitcoin.wallet import CBitcoinSecret
from fixtures import whatthefee, postgres_factory, swapd_factory
from pyln.testing.fixtures import (
    bitcoind,
    directory,
    db_provider,
    executor,
    jsonschemas,
    node_cls,
    node_factory,
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
    "db_provider",
    "executor",
    "jsonschemas",
    "node_cls",
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
    swapper.lightning_node.openchannel(user, 1000000)
    wait_for(
        lambda: all(
            channel["state"] == "CHANNELD_NORMAL"
            for channel in swapper.lightning_node.rpc.listpeerchannels()["channels"]
        )
    )
    return user, swapper


def add_fund_init(user, swapper, amount=100_000_000):
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    secret_key = CBitcoinSecret.from_secret_bytes(os.urandom(32))
    public_key = secret_key.pub
    add_fund_resp = swapper.rpc.add_fund_init(user, public_key, h)
    payment_request = user.rpc.invoice(
        amount,
        "test",
        "test",
        preimage=hexlify(preimage).decode("ASCII"),
    )["bolt11"]
    return add_fund_resp.address, payment_request, hexlify(h).decode("ASCII")

from binascii import hexlify
from bitcoinutils.keys import PrivateKey
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
    "setup_user_router_swapper",
    "setup_user_and_swapper",
    "create_swap_no_invoice_extended",
    "create_swap_no_invoice",
    "create_swap_extended",
    "create_swap",
    "whatthefee",
    "postgres_factory",
    "swapd_factory",
    "lock_time",
    "min_claim_blocks",
    "min_viable_cltv",
    "cltv_delta",
]


def setup_user_router_swapper(node_factory, swapd_factory, swapd_opts=None):
    user = node_factory.get_node()
    router = node_factory.get_node()
    swapper = swapd_factory.get_swapd(options=swapd_opts)
    swapper.lightning_node.open_channel(router, 1000000)
    router.open_channel(user, 1000000)
    return user, router, swapper


def setup_user_and_swapper(node_factory, swapd_factory, swapd_opts=None):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd(options=swapd_opts)
    swapper.lightning_node.open_channel(user, 1000000)
    return user, swapper


def create_swap_no_invoice_extended(user: ClnNode, swapper: SwapdServer):
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    refund_privkey = PrivateKey()
    refund_pubkey = refund_privkey.get_public_key().to_hex()
    create_swap_resp = swapper.rpc.create_swap(user, refund_pubkey, h)
    return (
        create_swap_resp.address,
        preimage.hex(),
        h.hex(),
        refund_privkey,
        create_swap_resp.claim_pubkey,
        create_swap_resp.lock_height,
    )


def create_swap_no_invoice(user: ClnNode, swapper: SwapdServer):
    address, preimage, h, _, _, _ = create_swap_no_invoice_extended(user, swapper)
    return address, preimage, h


def create_swap_extended(user: ClnNode, swapper: SwapdServer, amount=100_000_000):
    address, preimage, h, refund_privkey, claim_pubkey, lock_height = (
        create_swap_no_invoice_extended(user, swapper)
    )
    payment_request = user.create_invoice(
        amount,
        description="test",
        preimage=preimage,
    )
    return address, payment_request, h, refund_privkey, claim_pubkey, lock_height


def create_swap(user: ClnNode, swapper: SwapdServer, amount=100_000_000):
    address, payment_request, h, _, _, _ = create_swap_extended(user, swapper, amount)
    return address, payment_request, h

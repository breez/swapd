from binascii import hexlify
from bitcoin.wallet import CBitcoinSecret
from pyln.testing.fixtures import *
from pyln.testing.utils import wait_for

import hashlib
import helpers
import os


def test_swap_success(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    swapper.lightning_node.openchannel(user, 1000000)
    wait_for(
        lambda: all(
            channel["state"] == "CHANNELD_NORMAL"
            for channel in swapper.lightning_node.rpc.listpeerchannels()["channels"]
        )
    )
    user_node_id = user.info["id"]
    secret_key = CBitcoinSecret.from_secret_bytes(os.urandom(32))
    public_key = secret_key.pub
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    add_fund_resp = swapper.rpc.add_fund_init(user, public_key, h)
    txid = user.bitcoin.rpc.sendtoaddress(add_fund_resp.address, 100_000)
    user.bitcoin.generate_block(1)

    # TODO: Add this method
    wait_for(
        lambda: swapper.internal_grpc.get_swap(add_fund_resp.address)[
            "confirmation_height"
        ]
        > 0
    )

    payment_request = user.rpc.invoice(
        100_000_000, "swap-success", "swap-success", preimage=hexlify(preimage)
    )["bolt11"]
    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=hexlify(h))["invoices"][0]["status"]
        == "paid"
    )

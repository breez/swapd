from binascii import hexlify
from bitcoin.wallet import CBitcoinSecret
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
from pyln.testing.utils import wait_for
from fixtures import whatthefee, swapd_factory, postgres_factory
import grpc
import hashlib
import os


def test_pay_again_fails(node_factory, swapd_factory):
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
    txid = user.bitcoin.rpc.sendtoaddress(add_fund_resp.address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(
        lambda: len(swapper.internal_rpc.get_swap(add_fund_resp.address).outputs) > 0
    )

    payment_request = user.rpc.invoice(
        100_000_000,
        "swap-success",
        "swap-success",
        preimage=hexlify(preimage).decode("ASCII"),
    )["bolt11"]
    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=hexlify(h).decode("ASCII"))[
            "invoices"
        ][0]["status"]
        == "paid"
    )
    try:
        swapper.rpc.get_swap_payment(payment_request)
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "swap already paid"
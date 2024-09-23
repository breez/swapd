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
from fixtures import swapd_factory, postgres_factory
import hashlib
import os
import grpc


def test_filtered_address(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    swapper.lightning_node.openchannel(user, 1000000)
    wait_for(
        lambda: all(
            channel["state"] == "CHANNELD_NORMAL"
            for channel in swapper.lightning_node.rpc.listpeerchannels()["channels"]
        )
    )
    user_address = user.rpc.newaddr()["bech32"]
    # create 2 utxos, because the first will be needed as reserve 
    user_address, user_txid = user.fundwallet(200_000)
    swapper.internal_rpc.add_address_filters([user_address])

    user_node_id = user.info["id"]
    secret_key = CBitcoinSecret.from_secret_bytes(os.urandom(32))
    public_key = secret_key.pub
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    add_fund_resp = swapper.rpc.add_fund_init(user, public_key, h)
    txid = user.rpc.withdraw(add_fund_resp.address, 100_000)["txid"]
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

    try:
        swapper.rpc.get_swap_payment(payment_request)
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "confirmed utxo values don't match invoice value"

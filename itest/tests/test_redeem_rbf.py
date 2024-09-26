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
from decimal import Decimal
from pyln.testing.utils import wait_for
from fixtures import whatthefee, swapd_factory, postgres_factory
import hashlib
import os


def test_redeem_rbf_close_to_deadline(node_factory, swapd_factory):
    user = node_factory.get_node()

    # slow down the redeem poll interval, so the replacement transaction is not
    # mined during the creation of new blocks.
    swapper = swapd_factory.get_swapd(
        options={
            "redeem-poll-interval-seconds": "4",
        }
    )
    swapper.lightning_node.openchannel(user, 1000000)
    wait_for(
        lambda: all(
            channel["state"] == "CHANNELD_NORMAL"
            for channel in swapper.lightning_node.rpc.listpeerchannels()["channels"]
        )
    )
    expected_outputs = len(swapper.lightning_node.rpc.listfunds()["outputs"]) + 1
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
        "redeem-success",
        "redeem-success",
        preimage=hexlify(preimage).decode("ASCII"),
    )["bolt11"]
    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=hexlify(h).decode("ASCII"))[
            "invoices"
        ][0]["status"]
        == "paid"
    )

    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    redeem_txid1 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    redeem_raw1 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
        redeem_txid1, True
    )
    assert redeem_raw1["vin"][0]["txid"] == txid
    assert redeem_raw1["vout"][0]["value"] == Decimal("0.00099755")

    # Set the effective fee rate of the mempool tx to 0, so it won't be mined
    swapper.lightning_node.bitcoin.rpc.prioritisetransaction(
        redeem_txid1, None, -1000000
    )
    orig_len = swapper.lightning_node.bitcoin.rpc.getblockcount()
    swapper.lightning_node.bitcoin.generate_block(288)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        return memp[0] != redeem_txid1

    wait_for(check_bumped)
    redeem_txid2 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    redeem_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
        redeem_txid2, True
    )
    assert redeem_raw2["vin"][0]["txid"] == txid
    assert redeem_raw2["vout"][0]["value"] == Decimal("0.00099635")

    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(
        lambda: len(swapper.lightning_node.rpc.listfunds()["outputs"])
        == expected_outputs
    )


def test_redeem_rbf_new_feerate(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    swapper.lightning_node.openchannel(user, 1000000)
    wait_for(
        lambda: all(
            channel["state"] == "CHANNELD_NORMAL"
            for channel in swapper.lightning_node.rpc.listpeerchannels()["channels"]
        )
    )
    expected_outputs = len(swapper.lightning_node.rpc.listfunds()["outputs"]) + 1
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
        "redeem-success",
        "redeem-success",
        preimage=hexlify(preimage).decode("ASCII"),
    )["bolt11"]
    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=hexlify(h).decode("ASCII"))[
            "invoices"
        ][0]["status"]
        == "paid"
    )

    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    redeem_txid1 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    redeem_raw1 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
        redeem_txid1, True
    )
    assert redeem_raw1["vin"][0]["txid"] == txid
    assert redeem_raw1["vout"][0]["value"] == Decimal("0.00099755")
    # Set the effective fee rate of the mempool tx to 0, so it won't be mined
    swapper.lightning_node.bitcoin.rpc.prioritisetransaction(
        redeem_txid1, None, -1000000
    )

    # increase the current feerates by 10x (it's an exponent, so won't be 10x)
    swapper.whatthefee.magnify(5)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        return memp[0] != redeem_txid1

    wait_for(check_bumped)
    redeem_txid2 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    redeem_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
        redeem_txid2, True
    )
    assert redeem_raw2["vin"][0]["txid"] == txid
    assert redeem_raw2["vout"][0]["value"] == Decimal("0.00097303")

    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(
        lambda: len(swapper.lightning_node.rpc.listfunds()["outputs"])
        == expected_outputs
    )

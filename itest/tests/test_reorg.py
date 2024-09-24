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
import hashlib
import os


def test_reorg(node_factory, swapd_factory):
    # First initiate a new swap
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    bitcoin = swapper.lightning_node.bitcoin
    expected_outputs = len(swapper.lightning_node.rpc.listfunds()["outputs"]) + 1
    user_node_id = user.info["id"]
    secret_key = CBitcoinSecret.from_secret_bytes(os.urandom(32))
    public_key = secret_key.pub
    preimage = os.urandom(32)
    h = hashlib.sha256(preimage).digest()
    add_fund_resp = swapper.rpc.add_fund_init(user, public_key, h)

    # Send funds to the address and confirm the tx
    txid = user.bitcoin.rpc.sendtoaddress(add_fund_resp.address, 100_000 / 10**8)
    bitcoin.generate_block(1)

    # Ensure the swap transaction is picked up by swapd.
    wait_for(
        lambda: len(swapper.internal_rpc.get_swap(add_fund_resp.address).outputs) > 0
    )

    fee_delta = 1000000
    orig_len = bitcoin.rpc.getblockcount()
    old_hash = bitcoin.rpc.getblockhash(orig_len)

    # Invalidate the block containg the swap transaction
    bitcoin.rpc.invalidateblock(old_hash)
    bitcoin.wait_for_log(
        r"InvalidChainFound: invalid block=.*  height={}".format(orig_len)
    )
    memp = bitcoin.rpc.getrawmempool()
    for txid in memp:
        # Set the effective fee rate of the mempool tx to 0, so it won't be mined
        bitcoin.rpc.prioritisetransaction(txid, None, -fee_delta)

    # This is the reorg, 2 blocks, to ensure the new chain is longer than the
    # old one. The new chain will not contain the swap transaction.
    bitcoin.generate_block(2)
    bitcoin.wait_for_log(r"UpdateTip: new best=.* height={}".format(orig_len + 1))

    # The swap should now (soon) no longer contain the no longer confirmed tx.
    wait_for(
        lambda: len(swapper.internal_rpc.get_swap(add_fund_resp.address).outputs) == 0
    )

    for txid in memp:
        # restore priority so they are mined
        bitcoin.rpc.prioritisetransaction(txid, None, fee_delta)

    # This new block will contain the swap transaction again
    bitcoin.generate_block(1)

    # Ensure the swapper picks it up.
    wait_for(
        lambda: len(swapper.internal_rpc.get_swap(add_fund_resp.address).outputs) > 0
    )

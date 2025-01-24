from helpers import *
from decimal import Decimal


def test_claim_rbf_close_to_deadline(node_factory, swapd_factory, lock_time):
    # slow down the claim poll interval, so the replacement transaction is not
    # mined during the creation of new blocks.
    interval = "4"
    if SLOW_MACHINE:
        interval = "20"

    user, swapper = setup_user_and_swapper(
        node_factory,
        swapd_factory,
        {
            "claim-poll-interval-seconds": interval,
        },
    )
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h = create_swap(user, swapper)
    txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])

    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    claim_txid1 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    claim_raw1 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(claim_txid1, True)
    assert claim_raw1["vin"][0]["txid"] == txid
    assert claim_raw1["vout"][0]["value"] == Decimal("0.00099586")

    # Set the effective fee rate of the mempool tx to 0, so it won't be mined
    swapper.lightning_node.bitcoin.rpc.prioritisetransaction(
        claim_txid1, None, -1000000
    )
    swapper.lightning_node.bitcoin.generate_block(lock_time - 1)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        assert len(memp) == 1
        claim_txid2 = memp[0]
        if claim_txid2 == claim_txid1:
            return False

        claim_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
            claim_txid2, True
        )
        return claim_raw2["vout"][0]["value"] != claim_raw1["vout"][0]["value"]

    wait_for(check_bumped)
    claim_txid2 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    claim_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(claim_txid2, True)
    assert claim_raw2["vin"][0]["txid"] == txid
    assert claim_raw2["vout"][0]["value"] == Decimal("0.00099312")

    swapper.lightning_node.bitcoin.generate_block(1)

    def wait_outputs():
        utxos = swapper.lightning_node.list_utxos()
        return len(utxos) == expected_outputs

    wait_for(wait_outputs)


def test_claim_rbf_new_feerate(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h = create_swap(user, swapper)
    txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])

    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    claim_txid1 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    claim_raw1 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(claim_txid1, True)
    assert claim_raw1["vin"][0]["txid"] == txid
    assert claim_raw1["vout"][0]["value"] == Decimal("0.00099586")
    # Set the effective fee rate of the mempool tx to 0, so it won't be mined
    swapper.lightning_node.bitcoin.rpc.prioritisetransaction(
        claim_txid1, None, -1000000
    )

    # increase the current feerates by 5x (it's an exponent, so won't be 5x)
    swapper.whatthefee.magnify(5)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        assert len(memp) == 1
        claim_txid2 = memp[0]
        if claim_txid2 == claim_txid1:
            return False

        claim_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
            claim_txid2, True
        )
        return claim_raw2["vout"][0]["value"] != claim_raw1["vout"][0]["value"]

    wait_for(check_bumped)
    claim_txid2 = swapper.lightning_node.bitcoin.rpc.getrawmempool()[0]
    claim_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(claim_txid2, True)
    assert claim_raw2["vin"][0]["txid"] == txid
    assert claim_raw2["vout"][0]["value"] == Decimal("0.00097933")

    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)

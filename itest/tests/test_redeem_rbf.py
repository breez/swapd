from helpers import *
from decimal import Decimal


def test_redeem_rbf_close_to_deadline(node_factory, swapd_factory):
    # slow down the redeem poll interval, so the replacement transaction is not
    # mined during the creation of new blocks.
    interval = "4"
    if SLOW_MACHINE:
        interval = "20"

    user, swapper = setup_user_and_swapper(
        node_factory,
        swapd_factory,
        {
            "redeem-poll-interval-seconds": interval,
        },
    )
    expected_outputs = len(swapper.lightning_node.rpc.listfunds()["outputs"]) + 1
    address, payment_request, h = add_fund_init(user, swapper)
    txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=h)["invoices"][0]["status"] == "paid"
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
    swapper.lightning_node.bitcoin.generate_block(288)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        assert len(memp) == 1
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
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    expected_outputs = len(swapper.lightning_node.rpc.listfunds()["outputs"]) + 1
    address, payment_request, h = add_fund_init(user, swapper)
    txid = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.get_swap_payment(payment_request)
    wait_for(
        lambda: user.rpc.listinvoices(payment_hash=h)["invoices"][0]["status"] == "paid"
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

    # increase the current feerates by 5x (it's an exponent, so won't be 5x)
    swapper.whatthefee.magnify(5)

    def check_bumped():
        memp = swapper.lightning_node.bitcoin.rpc.getrawmempool()
        if len(memp) == 0:
            return False
        assert len(memp) == 1
        redeem_txid2 = memp[0]
        if redeem_txid2 == redeem_txid1:
            return False

        redeem_raw2 = swapper.lightning_node.bitcoin.rpc.getrawtransaction(
            redeem_txid2, True
        )
        return redeem_raw2["vout"][0]["value"] != redeem_raw1["vout"][0]["value"]

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

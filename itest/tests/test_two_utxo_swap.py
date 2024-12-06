from helpers import *


def test_two_utxo_swap(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h = create_swap(user, swapper, amount=200_000_000)
    txid1 = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    txid2 = user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) == 2)

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])

    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)

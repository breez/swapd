from helpers import *


def test_swap_payout_success(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, payment_request, h = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])

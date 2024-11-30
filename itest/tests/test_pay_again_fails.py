from helpers import *
import grpc


def test_pay_again_fails(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, payment_request, h = add_fund_init(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    swapper.rpc.get_swap_payment(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])

    try:
        swapper.rpc.get_swap_payment(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "swap already paid"

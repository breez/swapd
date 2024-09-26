from helpers import *
import grpc


def test_pay_after_deadline_fails(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    address, payment_request, _ = add_fund_init(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    height = user.bitcoin.rpc.getblockcount()
    user.bitcoin.generate_block(216)
    user.bitcoin.wait_for_log(r"UpdateTip: new best=.* height={}".format(height + 216))

    try:
        swapper.rpc.get_swap_payment(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "confirmed utxo values don't match invoice value"

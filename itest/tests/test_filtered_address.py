from helpers import *
import grpc


def test_filtered_address(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    user_address, user_txid = user.fund_wallet(200_000)
    swapper.internal_rpc.add_address_filters([user_address])

    address, payment_request, h, preimage = create_swap(user, swapper)
    user.send_onchain(address, 100_000, confirm=1)
    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    try:
        swapper.rpc.pay_swap(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "confirmed utxo values don't match invoice value"

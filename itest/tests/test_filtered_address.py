from helpers import *
import grpc


def test_filtered_address(node_factory, swapd_factory):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    user_address = user.rpc.newaddr()["bech32"]
    user_address, user_txid = user.fundwallet(200_000)
    swapper.internal_rpc.add_address_filters([user_address])

    address, payment_request, _ = add_fund_init(user, swapper)
    txid = user.rpc.withdraw(address, 100_000)["txid"]
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    try:
        swapper.rpc.get_swap_payment(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "confirmed utxo values don't match invoice value"

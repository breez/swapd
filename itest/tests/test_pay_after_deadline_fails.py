from helpers import *
import grpc


def test_pay_after_deadline_fails(
    node_factory, swapd_factory, lock_time, min_claim_blocks
):
    user = node_factory.get_node()
    swapper = swapd_factory.get_swapd()
    address, payment_request, h, preimage = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    height = user.bitcoin.rpc.getblockcount()
    blocks_to_add = lock_time - min_claim_blocks - 1
    user.bitcoin.generate_block(blocks_to_add)
    user.bitcoin.wait_for_log(
        r"UpdateTip: new best=.* height={}".format(height + blocks_to_add)
    )

    try:
        swapper.rpc.pay_swap(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "swap expired"

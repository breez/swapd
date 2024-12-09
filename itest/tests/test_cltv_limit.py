from helpers import *
import grpc


def test_below_cltv_limit(
    node_factory, swapd_factory, lock_time, min_claim_blocks, min_viable_cltv
):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, preimage, payment_hash = create_swap_no_invoice(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    payment_request = user.create_invoice(
        100_000_000,
        description="test",
        preimage=preimage,
        cltv=lock_time - 1 - min_claim_blocks - min_viable_cltv + 1,
    )

    try:
        swapper.rpc.pay_swap(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        assert e.details() == "swap expired"


def test_on_cltv_limit(
    node_factory, swapd_factory, lock_time, min_claim_blocks, min_viable_cltv
):
    user, swapper = setup_user_and_swapper(node_factory, swapd_factory)
    address, preimage, payment_hash = create_swap_no_invoice(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    payment_request = user.create_invoice(
        100_000_000,
        description="test",
        preimage=preimage,
        cltv=lock_time - 1 - min_claim_blocks - min_viable_cltv,
    )

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=payment_hash)[0]["paid"])


def test_below_cltv_limit_with_router(
    node_factory, swapd_factory, lock_time, min_claim_blocks, cltv_delta
):
    user, router, swapper = setup_user_router_swapper(
        node_factory, swapd_factory, swapd_opts={"min-viable-cltv": 0}
    )
    address, preimage, payment_hash = create_swap_no_invoice(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    payment_request = user.create_invoice(
        100_000_000,
        description="test",
        preimage=preimage,
        cltv=lock_time - 1 - min_claim_blocks - cltv_delta + 1,
    )

    try:
        swapper.rpc.pay_swap(payment_request)
        assert False
    except grpc._channel._InactiveRpcError as e:
        # TODO: Ideally this would return a more useful error for the user.
        #       The user is too far away for the cltv timeout.
        assert e.details() == "payment failed"


def test_on_cltv_limit_with_router(
    node_factory, swapd_factory, lock_time, min_claim_blocks, cltv_delta
):
    user, router, swapper = setup_user_router_swapper(
        node_factory, swapd_factory, swapd_opts={"min-viable-cltv": 0}
    )
    address, preimage, payment_hash = create_swap_no_invoice(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    # NOTE: the '- 3' is because LND adds 3 to the cltv delta. Otherwise it would be -1
    payment_request = user.create_invoice(
        100_000_000,
        description="test",
        preimage=preimage,
        cltv=lock_time - 1 - min_claim_blocks - cltv_delta - 3,
    )

    swapper.rpc.pay_swap(payment_request)
    wait_for(lambda: user.list_invoices(payment_hash=payment_hash)[0]["paid"])

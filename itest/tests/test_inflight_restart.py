from helpers import *
import grpc
import pytest
import threading


def test_inflight_restart_swapd_payment_success(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(
        node_factory, swapd_factory, hodl_plugin=True
    )
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h, preimage = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    def pay_swap_in_background():
        try:
            swapper.rpc.pay_swap(payment_request)
        except Exception as e:
            pass

    threading.Thread(target=pay_swap_in_background).start()

    # The user will hold the htlc until resolve is called
    wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
    swapper.restart(timeout=1, may_fail=True)

    def get_info_works():
        try:
            swapper.internal_rpc.get_info()
            return True
        except Exception as e:
            return False

    wait_for(get_info_works)
    user.call("resolve", {})

    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])
    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)


def test_inflight_restart_swapd_payment_retry(node_factory, swapd_factory):
    # TODO: swapd exits with exit code -15 at the end of this test. This is because
    # there is still a grpc client connected, causing the public grc server to not
    # stop gracefully.
    user, swapper = setup_user_and_swapper(
        node_factory, swapd_factory, hodl_plugin=True, swapd_may_fail=True
    )
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h, preimage = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    future = swapper.rpc.pay_swap_future(payment_request)

    # The user will hold the htlc until resolve is called
    wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
    swapper.restart(timeout=1, may_fail=True)
    future.cancel()

    def get_info_works():
        try:
            swapper.internal_rpc.get_info()
            return True
        except Exception:
            return False

    wait_for(get_info_works)
    user.call(
        "resolve",
        {"index": -1, "result": {"result": "fail", "failure_message": "2002"}},
    )

    wait_for(lambda: len(user.list_peerchannels()[0]["htlcs"]) == 0)

    def resolve_when_received(user):
        wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
        user.call("resolve", {})

    def pay_swap_after_unlock():
        try:
            swapper.rpc.pay_swap(payment_request)
            return True
        except grpc._channel._InactiveRpcError as e:
            if e.details() != "swap is locked":
                raise e
            return False

    threading.Thread(target=resolve_when_received, args=(user,)).start()
    wait_for(pay_swap_after_unlock)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])
    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)


def test_inflight_restart_cln_payment_success(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(
        node_factory, swapd_factory, hodl_plugin=True
    )
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h, preimage = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    def pay_swap_in_background():
        try:
            swapper.rpc.pay_swap(payment_request)
        except Exception as e:
            pass

    threading.Thread(target=pay_swap_in_background).start()

    # The user will hold the htlc until resolve is called
    wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
    swapper.lightning_node.restart()

    user.connect(swapper.lightning_node)

    def reconnected():
        c = user.list_peerchannels()[0]
        return c["peer_connected"] and c["reestablished"]

    wait_for(reconnected)

    user.call("resolve", {})

    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])
    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)


@pytest.mark.skip(
    reason="restarting cln when a payment is in-flight keeps the swap locked for now. It would have to be manually unlocked."
)
def test_inflight_restart_cln_payment_retry(node_factory, swapd_factory):
    user, swapper = setup_user_and_swapper(
        node_factory, swapd_factory, hodl_plugin=True, swapd_may_fail=True
    )
    expected_outputs = len(swapper.lightning_node.list_utxos()) + 1
    address, payment_request, h, preimage = create_swap(user, swapper)
    user.bitcoin.rpc.sendtoaddress(address, 100_000 / 10**8)
    user.bitcoin.generate_block(1)

    wait_for(lambda: len(swapper.internal_rpc.get_swap(address).outputs) > 0)

    future = swapper.rpc.pay_swap_future(payment_request)

    # The user will hold the htlc until resolve is called
    wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
    swapper.lightning_node.restart(timeout=1)
    future.cancel()

    user.connect(swapper.lightning_node)

    def reconnected():
        c = user.list_peerchannels()[0]
        return c["peer_connected"] and c["reestablished"]

    wait_for(reconnected)
    user.call(
        "resolve",
        {"index": -1, "result": {"result": "fail", "failure_message": "2002"}},
    )

    wait_for(lambda: len(user.list_peerchannels()[0]["htlcs"]) == 0)

    def resolve_when_received(user):
        wait_for(lambda: user.call("hodl_count", {})["count"] > 0)
        user.call("resolve", {})

    threading.Thread(target=resolve_when_received, args=(user,)).start()

    def pay_swap_after_unlock():
        try:
            swapper.rpc.pay_swap(payment_request)
            return True
        except grpc._channel._InactiveRpcError as e:
            if e.details() != "swap is locked":
                raise e
            return False

    wait_for(pay_swap_after_unlock)
    wait_for(lambda: user.list_invoices(payment_hash=h)[0]["paid"])
    wait_for(lambda: swapper.lightning_node.bitcoin.rpc.getmempoolinfo()["size"] == 1)
    swapper.lightning_node.bitcoin.generate_block(1)
    wait_for(lambda: len(swapper.lightning_node.list_utxos()) == expected_outputs)

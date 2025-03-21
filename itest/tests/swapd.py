from collections import OrderedDict
from pyln.testing.utils import (
    TailableProc,
    TEST_NETWORK,
    SLOW_MACHINE,
    TIMEOUT,
    BITCOIND_CONFIG,
    drop_unused_port,
    reserve_unused_port,
    wait_for,
)
from swap_pb2_grpc import TaprootSwapperStub
from swap_internal_pb2_grpc import SwapManagerStub
from pathlib import Path
from flask import Flask, request
from cheroot.wsgi import Server
from cheroot.wsgi import PathInfoDispatcher
import flask
import swap_internal_pb2
import swap_pb2

import multiprocessing
import grpc
import logging
import math
import os
import threading

SWAPD_CONFIG = OrderedDict(
    {
        "log-level": "swapd=trace,info",
        "chain-poll-interval-seconds": "1",
        "claim-poll-interval-seconds": "1",
        "payment-poll-interval-seconds": "1",
        "preimage-poll-interval-seconds": "1",
        "whatthefee-poll-interval-seconds": "1",
        "max-swap-amount-sat": "4000000",
        "lock-time": "288",
        "min-confirmations": "1",
        "min-claim-blocks": "72",
        "dust-limit-sat": "546",
    }
)


class SwapD(TailableProc):
    def __init__(
        self,
        lightning_node,
        whatthefee,
        process_dir,
        bitcoindproxy,
        db_url,
        grpc_port=27103,
        internal_grpc_port=27104,
        swapd_id=0,
        fees=[1, 2, 3, 4, 5],
    ):
        # We handle our own version of verbose, below.
        TailableProc.__init__(self, process_dir, verbose=True)
        self.executable = "swapd"
        self.grpc_port = grpc_port
        self.internal_grpc_port = internal_grpc_port
        self.bitcoindproxy = bitcoindproxy
        self.lightning_node = lightning_node
        self.prefix = "swapd-%d" % (swapd_id)
        self.process_dir = process_dir
        self.opts = SWAPD_CONFIG.copy()
        self.logger = logging.getLogger("SwapD")

        opts = {
            "address": "127.0.0.1:{}".format(grpc_port),
            "internal-address": "127.0.0.1:{}".format(internal_grpc_port),
            "network": TEST_NETWORK,
            "bitcoind-rpc-user": BITCOIND_CONFIG["rpcuser"],
            "bitcoind-rpc-password": BITCOIND_CONFIG["rpcpassword"],
            "db-url": db_url,
            "auto-migrate": None,
            "whatthefee-url": "http://127.0.0.1:{}?fees={}".format(
                whatthefee.port, "%2C".join(map(str, fees))
            ),
        }

        for k, v in opts.items():
            self.opts[k] = v

    @property
    def cmd_line(self):

        opts = []
        for k, v in self.opts.items():
            if v is None:
                opts.append("--{}".format(k))
            elif isinstance(v, list):
                for i in v:
                    opts.append("--{}={}".format(k, i))
            else:
                opts.append("--{}={}".format(k, v))

        cmd = [self.executable] + opts
        return cmd

    def start(self, stdin=None, wait_for_initialized=True, stderr_redir=False):
        self.opts["bitcoind-rpc-address"] = "http://127.0.0.1:{}".format(
            self.bitcoindproxy.rpcport
        )
        self.logger.debug(
            "starting swapd with commandline: '{}'".format(" ".join(self.cmd_line))
        )
        TailableProc.start(self, stdin, stdout_redir=True, stderr_redir=stderr_redir)
        if wait_for_initialized:
            self.wait_for_log("swapd started")
        self.logger.info("SwapD started")

    def wait(self, timeout=TIMEOUT):
        """Wait for the daemon to stop for up to timeout seconds

        Returns the returncode of the process, None if the process did
        not return before the timeout triggers.
        """
        self.proc.wait(timeout)
        return self.proc.returncode


class SwapdServer(object):
    def __init__(
        self,
        swapd_id,
        process_dir,
        bitcoind,
        whatthefee,
        lightning_node,
        db_url,
        may_fail=False,
        grpc_port=None,
        internal_grpc_port=None,
        options=None,
        fees=[1, 2, 3, 4, 5],
    ):
        self.bitcoind = bitcoind
        self.lightning_node = lightning_node
        self.whatthefee = whatthefee
        self.may_fail = may_fail
        self.rc = 0
        self._create_grpc_rpc(grpc_port)
        self._create_internal_grpc_rpc(internal_grpc_port)
        self.logger = logging.getLogger("SwapdServer")

        self.daemon = SwapD(
            lightning_node,
            whatthefee,
            process_dir,
            bitcoind.get_proxy(),
            db_url,
            grpc_port=grpc_port,
            internal_grpc_port=internal_grpc_port,
            swapd_id=swapd_id,
            fees=fees,
        )

        if options is not None:
            self.daemon.opts.update(options)

    def _create_grpc_rpc(self, port):
        if port is None:
            self.grpc_port = reserve_unused_port()
        else:
            self.grpc_port = port

        # Now the node will actually start up and use them, so we can
        # create the RPC instance.
        self.rpc = SwapperGrpc(
            host="127.0.0.1",
            port=self.grpc_port,
        )

    def _create_internal_grpc_rpc(self, port):
        if port is None:
            self.internal_grpc_port = reserve_unused_port()
        else:
            self.internal_grpc_port = port

        # Now the node will actually start up and use them, so we can
        # create the RPC instance.
        self.internal_rpc = SwapManagerGrpc(
            host="127.0.0.1",
            port=self.internal_grpc_port,
        )

    @property
    def grpc(self):
        """Tiny helper to return a grpc stub."""
        # Before doing anything let's see if we have a grpc-port at all
        address = filter(
            lambda v: v[0] == "address", self.daemon.opts.items()
        ).__next__()[1]
        channel = grpc.insecure_channel(
            address,
        )
        return TaprootSwapperStub(channel)

    @property
    def internal_grpc(self):
        """Tiny helper to return an internal grpc stub."""
        # Before doing anything let's see if we have a grpc-port at all
        address = filter(
            lambda v: v[0] == "internal-address", self.daemon.opts.items()
        ).__next__()[1]
        channel = grpc.insecure_channel(
            address,
        )
        return SwapManagerStub(channel)

    def start(self, stderr_redir=False, wait_for_bitcoind_sync=True):
        self.rc = 0
        self.daemon.start(stderr_redir=stderr_redir)
        if wait_for_bitcoind_sync:
            wait_for(self.is_synced)
            self.logger.debug("swapd is synced")
        self.logger.debug("swapd is started")

    def is_synced(self):
        height = self.bitcoind.rpc.getblockchaininfo()["blocks"]
        try:
            block_height = self.internal_rpc.get_info().block_height
            self.logger.debug(
                f"chain height is {height}, swapd height is {block_height}"
            )
            return block_height == height
        except Exception as e:
            self.logger.debug(f"still waiting for sync: {e}")
            return False

    def stop(self, timeout=10, may_fail=False):
        """Attempt to do a clean shutdown, but kill if it hangs"""

        # Tell the daemon to stop
        try:
            # May fail if the process already died
            self.internal_rpc.stop()
        except Exception:
            pass

        try:
            self.rc = self.daemon.wait(timeout)
        except Exception as e:
            self.rc = None
            self.logger.debug(f"Error waiting for swapd to stop: {e}")
            pass

        # If it did not stop be more insistent
        if self.rc is None:
            self.rc = self.daemon.stop()

        if self.rc != 0 and not may_fail and not self.may_fail:
            raise ValueError("Swapd did not exit cleanly, rc={}".format(self.rc))
        else:
            return self.rc

    def restart(self, timeout=10, clean=True, may_fail=False):
        """Stop and restart the swapd node.

        Keyword arguments:
        timeout: number of seconds to wait for a shutdown
        clean: whether to issue a `stop` RPC command before killing
        """
        if clean:
            self.stop(timeout, may_fail)
        else:
            rc = self.daemon.stop()
            if not may_fail and rc != 0:
                raise ValueError("Swapd did not exit cleanly, rc={}".format(rc))

        self.start()


def dump(obj):
    for attr in dir(obj):
        print("obj.%s = %r" % (attr, getattr(obj, attr)))


class SwapperGrpc(object):
    def __init__(
        self,
        host: str,
        port: int,
    ):
        self.logger = logging.getLogger("SwapGrpc")
        self.logger.debug(f"Connecting to grpc interface at {host}:{port}")
        self.channel = grpc.insecure_channel(f"{host}:{port}")
        self.stub = TaprootSwapperStub(self.channel)

    def create_swap(self, lightning_node, refund_pubkey, hash):
        node_id = lightning_node.info["id"]
        payload = swap_pb2.CreateSwapRequest(
            hash=hash, refund_pubkey=bytes.fromhex(refund_pubkey)
        )
        return self.stub.CreateSwap(payload)

    def pay_swap(self, payment_request):
        payload = swap_pb2.PaySwapRequest(payment_request=payment_request)
        return self.stub.PaySwap(payload)

    def pay_swap_future(self, payment_request):
        payload = swap_pb2.PaySwapRequest(payment_request=payment_request)
        return self.stub.PaySwap.future(payload)

    def refund_swap(self, address, transaction, input_index, pub_nonce):
        payload = swap_pb2.RefundSwapRequest(
            address=address,
            transaction=transaction,
            input_index=input_index,
            pub_nonce=pub_nonce,
        )
        return self.stub.RefundSwap(payload)


class SwapManagerGrpc(object):
    def __init__(
        self,
        host: str,
        port: int,
    ):
        self.logger = logging.getLogger("SwapManagerGrpc")
        self.logger.debug(f"Connecting to internal grpc interface at {host}:{port}")
        self.channel = grpc.insecure_channel(f"{host}:{port}")
        self.stub = SwapManagerStub(self.channel)

    def add_address_filters(self, addresses=[]):
        payload = swap_internal_pb2.AddAddressFiltersRequest(addresses=addresses)
        return self.stub.AddAddressFilters(payload)

    def get_info(self):
        payload = swap_internal_pb2.GetInfoRequest()
        return self.stub.GetInfo(payload)

    def get_swap(self, address=None):
        payload = swap_internal_pb2.GetSwapRequest(address=address)
        return self.stub.GetSwap(payload)

    def stop(self):
        payload = swap_internal_pb2.StopRequest()
        try:
            self.stub.Stop(payload)
        except Exception:
            pass


class SwapdFactory(object):
    """A factory to setup and start `swapd` daemons."""

    def __init__(
        self,
        testname,
        bitcoind,
        whatthefee,
        directory,
        postgres_factory,
        node_factory,
        options_provider,
        lock_time,
        min_claim_blocks,
        min_viable_cltv,
    ):

        self.testname = testname
        self.next_id = 1
        self.instances = []
        self.reserved_ports = []
        self.whatthefee = whatthefee
        self.bitcoind = bitcoind
        self.directory = directory
        self.lock = threading.Lock()
        self.postgres_factory = postgres_factory
        self.node_factory = node_factory
        self.options_provider = options_provider
        self.lock_time = lock_time
        self.min_claim_blocks = min_claim_blocks
        self.min_viable_cltv = min_viable_cltv

    def get_swapd_id(self):
        """Generate a unique numeric ID for a swapd instance"""
        with self.lock:
            swapd_id = self.next_id
            self.next_id += 1
            return swapd_id

    def get_swapd(
        self,
        options=None,
        fees=[1, 2, 3, 4, 5],
        start=True,
        expect_fail=False,
        may_fail=False,
        **kwargs,
    ):
        grpc_port = self.get_unused_port()
        internal_grpc_port = self.get_unused_port()
        swapd_id = self.get_swapd_id()
        process_dir = os.path.join(self.directory, "swapd-{}/".format(swapd_id))
        postgres = self.postgres_factory.get_container()
        node = self.node_factory.get_node()
        node_options = self.options_provider.get_options(node)

        base_options = {
            "lock-time": self.lock_time,
            "min-claim-blocks": self.min_claim_blocks,
            "min-viable-cltv": self.min_viable_cltv,
        }

        if options is None:
            options = base_options
        else:
            for k, v in options.items():
                base_options[k] = v
            options = base_options

        for k, v in node_options.items():
            options[k] = v

        swapd = SwapdServer(
            swapd_id,
            process_dir,
            self.bitcoind,
            self.whatthefee,
            node,
            postgres.connectionstring,
            may_fail,
            grpc_port,
            internal_grpc_port,
            options,
            fees,
        )

        self.instances.append(swapd)

        if start:
            try:
                swapd.start()
            except Exception:
                if expect_fail:
                    return swapd
                swapd.daemon.stop()
                raise
        return swapd

    def get_unused_port(self):
        port = reserve_unused_port()
        self.reserved_ports.append(port)
        return port

    def killall(self, expected_successes):
        """Returns true if every node we expected to succeed actually succeeded"""
        unexpected_fail = False
        err_msgs = []
        for i in range(len(self.instances)):
            try:
                self.instances[i].stop()
            except Exception:
                if expected_successes[i]:
                    unexpected_fail = True

        for p in self.reserved_ports:
            drop_unused_port(p)

        return not unexpected_fail, err_msgs


class WhatTheFee(object):
    def __init__(self, port=0):
        self.app = Flask("WhatTheFee")
        self.app.add_url_rule("/", "API entrypoint", self.get_fees, methods=["GET"])
        self.port = port
        self.request_count = 0
        self.quotient = 1
        self.logger = logging.getLogger("WhatTheFee")

    def get_fees(self):
        self.request_count += 1
        error = request.args.get("error")
        if error is not None:
            response = flask.Response(error)
            response.headers["Content-Type"] = "text/plain"
            response.status_code = 500
            return response

        # multiply the caller fees by the quotient.
        fees = ",".join(
            map(
                lambda x: str(round(math.log(int(x) * self.quotient) * 100)),
                request.args.get("fees").split(","),
            )
        )
        content = (
            '{"index": [3, 6, 9, 12, 18, 24, 36, 48, 72, 96, 144], '
            '"columns": ["0.0500", "0.2000", "0.5000", "0.8000", "0.9500"], '
            '"data": ['
        )
        for i in range(10):
            content += "[" + fees + "],"
        content += "[" + fees + "]]}"
        response = flask.Response(content)
        response.headers["Content-Type"] = "application/json"
        return response

    def start(self):
        d = PathInfoDispatcher({"/": self.app})
        self.server = Server(("0.0.0.0", self.port), d)
        self.proxy_thread = threading.Thread(target=self.server.start)
        self.proxy_thread.daemon = True
        self.proxy_thread.start()

        # Now that the whatthefee api is running on the real rpcport, let's tell
        # all future callers to talk to the proxyport. We use the bind_addr as a
        # signal that the port is bound and accepting connections.
        while self.server.bind_addr[1] == 0:
            pass
        self.port = self.server.bind_addr[1]
        self.logger.debug("WhatTheFee api listening on port {}".format(self.port))

    def stop(self):
        self.server.stop()
        self.proxy_thread.join()
        self.logger.debug(
            "WhatTheFee api shut down after processing {} requests".format(
                self.request_count
            )
        )

    def magnify(self, quotient=2):
        self.quotient = quotient


class ClnOptionsProvider(object):
    def __init__(self):
        pass

    def get_options(self, node):
        p = Path(node.node.daemon.lightning_dir) / TEST_NETWORK
        cert_path = p / "client.pem"
        key_path = p / "client-key.pem"
        ca_cert_path = p / "ca.pem"

        return {
            "cln-grpc-address": "https://localhost:{}".format(node.grpc_port),
            "cln-grpc-ca-cert": ca_cert_path.absolute().as_posix(),
            "cln-grpc-client-cert": cert_path.absolute().as_posix(),
            "cln-grpc-client-key": key_path.absolute().as_posix(),
        }


class LndOptionsProvider(object):
    def __init__(self):
        pass

    def get_options(self, node):
        p = Path(node.daemon.lnd_dir)
        ca_cert_path = p / "ca.cert"

        return {
            "lnd-grpc-address": "https://localhost:{}".format(node.grpc_port),
            "lnd-grpc-ca-cert": ca_cert_path.absolute().as_posix(),
            "lnd-grpc-macaroon": node.macaroon,
        }

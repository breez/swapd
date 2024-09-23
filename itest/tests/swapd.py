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
from swap_pb2_grpc import SwapperStub
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
import os
import threading

SWAPD_CONFIG = OrderedDict(
    {
        "log-level": "swapd=trace,sqlx::query=debug,reqwest=debug,info",
        "chain-poll-interval-seconds": "1",
        "redeem-poll-interval-seconds": "1",
        "preimage-poll-interval-seconds": "1",
        "max-swap-amount-sat": "4000000",
        "lock-time": "288",
        "min-confirmations": "1",
        "min-redeem-blocks": "72",
        "dust-limit-sat": "546",
    }
)


class SwapD(TailableProc):
    def __init__(
        self,
        lightning_node,
        whatthefee,
        node_grpc_port,
        process_dir,
        bitcoindproxy,
        db_url,
        grpc_port=27103,
        internal_grpc_port=27104,
        swapd_id=0,
        fees=[20, 40, 60, 80, 100],
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

        p = Path(lightning_node.daemon.lightning_dir) / TEST_NETWORK
        cert_path = p / "client.pem"
        key_path = p / "client-key.pem"
        ca_cert_path = p / "ca.pem"

        opts = {
            "address": "127.0.0.1:{}".format(grpc_port),
            "internal-address": "127.0.0.1:{}".format(internal_grpc_port),
            "network": TEST_NETWORK,
            "bitcoind-rpc-user": BITCOIND_CONFIG["rpcuser"],
            "bitcoind-rpc-password": BITCOIND_CONFIG["rpcpassword"],
            "cln-grpc-address": "https://localhost:{}".format(node_grpc_port),
            "cln-grpc-ca-cert": ca_cert_path.absolute().as_posix(),
            "cln-grpc-client-cert": cert_path.absolute().as_posix(),
            "cln-grpc-client-key": key_path.absolute().as_posix(),
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

        return [self.executable] + opts

    def start(self, stdin=None, wait_for_initialized=True, stderr_redir=False):
        self.opts["bitcoind-rpc-address"] = "http://127.0.0.1:{}".format(
            self.bitcoindproxy.rpcport
        )
        TailableProc.start(self, stdin, stdout_redir=True, stderr_redir=stderr_redir)
        if wait_for_initialized:
            self.wait_for_log("swapd started")
        logging.info("SwapD started")

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
        node_grpc_port,
        db_url,
        may_fail=False,
        grpc_port=None,
        internal_grpc_port=None,
        options=None,
        fees=[20, 40, 60, 80, 100],
        **kwargs,
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
            node_grpc_port,
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
        return SwapperStub(channel)

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
        self.daemon.start(stderr_redir=stderr_redir)
        if wait_for_bitcoind_sync:
            wait_for(self.is_synced)

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

    def stop(self, timeout=10):
        """Attempt to do a clean shutdown, but kill if it hangs"""

        # Tell the daemon to stop
        try:
            # May fail if the process already died
            self.internal_rpc.stop()
        except Exception:
            pass

        self.rc = self.daemon.wait(timeout)

        # If it did not stop be more insistent
        if self.rc is None:
            self.rc = self.daemon.stop()

        self.daemon.cleanup()

        if self.rc != 0 and not self.may_fail:
            raise ValueError("Node did not exit cleanly, rc={}".format(self.rc))
        else:
            return self.rc

    def restart(self, timeout=10, clean=True):
        """Stop and restart the lightning node.

        Keyword arguments:
        timeout: number of seconds to wait for a shutdown
        clean: whether to issue a `stop` RPC command before killing
        """
        if clean:
            self.stop(timeout)
        else:
            self.daemon.stop()

        self.start()


class SwapperGrpc(object):
    def __init__(
        self,
        host: str,
        port: int,
    ):
        self.logger = logging.getLogger("SwapGrpc")
        self.logger.debug(f"Connecting to grpc interface at {host}:{port}")
        self.channel = grpc.insecure_channel(f"{host}:{port}")
        self.stub = SwapperStub(self.channel)

    def add_fund_init(self, lightning_node, pubkey, hash):
        node_id = lightning_node.info["id"]
        payload = swap_pb2.AddFundInitRequest(nodeID=node_id, pubkey=pubkey, hash=hash)
        return self.stub.AddFundInit(payload)

    def get_swap_payment(self, payment_request):
        payload = swap_pb2.GetSwapPaymentRequest(paymentRequest=payment_request)
        return self.stub.GetSwapPayment(payload)


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
        executor,
        whatthefee,
        directory,
        node_factory,
        postgres_factory,
    ):

        self.testname = testname
        self.next_id = 1
        self.instances = []
        self.reserved_ports = []
        self.executor = executor
        self.whatthefee = whatthefee
        self.bitcoind = bitcoind
        self.directory = directory
        self.lock = threading.Lock()
        self.node_factory = node_factory
        self.postgres_factory = postgres_factory

    def get_swapd_id(self):
        """Generate a unique numeric ID for a swapd instance"""
        with self.lock:
            swapd_id = self.next_id
            self.next_id += 1
            return swapd_id

    def get_swapd(
        self,
        options=None,
        start=True,
        wait_for_bitcoind_sync=True,
        may_fail=False,
        expect_fail=False,
        cleandir=True,
        fees=[20, 40, 60, 80, 100],
        **kwargs,
    ):
        grpc_port = self.get_unused_port()
        internal_grpc_port = self.get_unused_port()
        cln_grpc_port = self.get_unused_port()
        swapd_id = self.get_swapd_id()
        process_dir = os.path.join(self.directory, "swapd-{}/".format(swapd_id))

        postgres = self.postgres_factory.get_container()
        node = self.node_factory.get_node(options={"grpc-port": cln_grpc_port})
        swapd = SwapdServer(
            swapd_id,
            process_dir,
            self.bitcoind,
            self.whatthefee,
            node,
            cln_grpc_port,
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
                swapd.start(wait_for_bitcoind_sync)
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

    def get_fees(self):
        self.request_count += 1
        error = request.args.get("error")
        if error is not None:
            response = flask.Response(error)
            response.headers["Content-Type"] = "text/plain"
            response.status_code = 500
            return response

        fees = request.args.get("fees")
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
        logging.debug("WhatTheFee api listening on port {}".format(self.port))

    def stop(self):
        self.server.stop()
        self.proxy_thread.join()
        logging.debug(
            "WhatTheFee api shut down after processing {} requests".format(
                self.request_count
            )
        )

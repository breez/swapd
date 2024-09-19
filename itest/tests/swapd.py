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
from pyln.testing.grpc import DUMMY_CA_PEM, DUMMY_CLIENT_PEM, DUMMY_CLIENT_KEY_PEM
import grpc

SWAPD_CONFIG = OrderedDict(
    {
        "log-level": "trace",
        "chain_poll_interval_seconds": "1",
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
        process_dir,
        grpc_port=27103,
        internal_grpc_port=27104,
        swapd_id=0,
    ):
        # We handle our own version of verbose, below.
        TailableProc.__init__(self, process_dir, verbose=False)
        self.executable = "swapd"
        self.grpc_port = grpc_port
        self.internal_grpc_port = internal_grpc_port
        self.bitcoindproxy = bitcoindproxy
        self.lightning_node = lightning_node
        self.prefix = "swapd-%d" % (swapd_id)
        self.opts = SWAPD_CONFIG.copy()
        opts = {
            "address": "127.0.0.1:{}".format(grpc_port),
            "internal-address": "127.0.0.1:{}".format(internal_grpc_port),
            "network": TEST_NETWORK,
            "bitcoind-rpc-user": BITCOIND_CONFIG["rpcuser"],
            "bitcoind-rpc-password": BITCOIND_CONFIG["rpcpassword"],
            "cln-grpc-ca-cert": DUMMY_CA_PEM,
            "cln-grpc-client-cert": DUMMY_CLIENT_PEM,
            "cln-grpc-client-key": DUMMY_CLIENT_KEY_PEM,
            "db-url": "",  # TODO: Add postgres
        }

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

        return self.cmd_prefix + [self.executable] + self.early_opts + opts

    def start(self, stdin=None, wait_for_initialized=True, stderr_redir=False):
        self.opts["bitcoind-rpc-address"] = "127.0.0.1:{}".format(self.rpcproxy.rpcport)
        TailableProc.start(self, stdin, stdout_redir=False, stderr_redir=stderr_redir)
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
        lightning_node,
        grpc_port=None,
        internal_grpc_port=None,
        options=None,
        **kwargs,
    ):
        self.bitcoin = bitcoind
        self.lightning_node = lightning_node

        self._create_grpc_rpc()
        self._create_internal_grpc_rpc()

        self.daemon = SwapD(
            lightning_node,
            process_dir,
            bitcoindproxy=bitcoind.get_proxy(),
            grpc_port=grpc_port,
            internal_grpc_port=internal_grpc_port,
            swapd_id=swapd_id,
        )

        if options is not None:
            self.daemon.opts.update(options)

    def _create_grpc_rpc(self):
        self.grpc_port = reserve_unused_port()

        # Now the node will actually start up and use them, so we can
        # create the RPC instance.
        self.rpc = SwapperGrpc(
            host="localhost",
            port=self.grpc_port,
        )

    def _create_internal_grpc_rpc(self):
        self.grpc_port = reserve_unused_port()

        # Now the node will actually start up and use them, so we can
        # create the RPC instance.
        self.internal_rpc = SwapManagerGrpc(
            host="localhost",
            port=self.grpc_port,
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
        return swap_pb2_grpc.SwapperStub(channel)

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
        return swap_internal_pb2_grpc.SwapManagerStub(channel)

    def start(self, stderr_redir=False, wait_for_bitcoind_sync=True):
        self.daemon.start(stderr_redir=stderr_redir)
        if wait_for_bitcoind_sync:
            height = self.bitcoind.rpc.getblockchaininfo()["blocks"]
            wait_for(lambda: self.internal_grpc.get_info()["block_height"] == height)

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
        self.stub = swap_pb2_grpc.SwapperStub(self.channel)

    def add_fund_init(self, lightning_node, pubkey, hash):
        node_id = lightning_node.info["id"]
        payload = swap_pb2.GetInfoRequest()
        return self.stub.AddFundInit(payload)

    def get_swap_payment(self, payment_request):
        payload = swap_pb2.GetSwapPaymentRequest(payment_request=payment_request)
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
        self.stub = swap_internal_pb2_grpc.SwapManagerStub(self.channel)

    def get_info(self):
        payload = swap_internal_pb2.GetInfoRequest()
        return self.stub.GetInfo(payload)

    def stop(self):
        payload = swap_internal_pb2.StopRequest()
        try:
            self.stub.Stop(payload)
        except Exception:
            pass


class SwapdFactory(object):
    """A factory to setup and start `swapd` daemons."""

    def __init__(
        self, testname, bitcoind, executor, directory, db_provider, node_factory
    ):

        self.testname = testname
        self.next_id = 1
        self.instances = []
        self.reserved_ports = []
        self.executor = executor
        self.bitcoind = bitcoind
        self.directory = directory
        self.lock = threading.Lock()
        self.db_provider = db_provider
        self.node_factory = node_factory

    def get_swapd_id(self):
        """Generate a unique numeric ID for a swapd instance"""
        with self.lock:
            swapd_id = self.next_id
            self.next_id += 1
            return swapd_id

    def get_swapd(
        self,
        options=None,
        dbfile=None,
        start=True,
        wait_for_bitcoind_sync=True,
        may_fail=False,
        expect_fail=False,
        cleandir=True,
        **kwargs,
    ):
        grpc_port = self.reserve_unused_port()
        internal_grpc_port = self.reserve_unused_port()
        swapd_id = self.get_swapd_id()
        process_dir = os.path.join(self.directory, "swapd-{}/".format(swapd_id))

        node = self.node_factory.get_node()
        swapd = SwapdServer(
            swapd_id,
            process_dir,
            self.bitcoind,
            node,
            grpc_port,
            internal_grpc_port,
            options,
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

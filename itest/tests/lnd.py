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

from pathlib import Path
from lightning_pb2_grpc import LightningStub
from walletunlocker_pb2_grpc import WalletUnlockerStub
from walletkit_pb2_grpc import WalletKitStub
import collections
import grpc
import logging
import lightning_pb2
import os
import shutil
import threading
import walletkit_pb2
import walletunlocker_pb2

FUNDAMOUNT = 10**6
LND_CONFIG = collections.OrderedDict(
    {
        "debuglevel": "debug",
        "nobootstrap": None,
        "trickledelay": 50,
        "keep-failed-payment-attempts": None,
        "bitcoin.node": "bitcoind",
        "bitcoin.regtest": None,
        "gossip.channel-update-interval": "10ms",
        "db.batch-commit-interval": "10ms",
        "maxbackoff": "1s",
        "norest": None,
    }
)


class LND(TailableProc):
    def __init__(self, lnd_dir, bitcoind, port=9735, grpc_port=None, node_id=0):
        # We handle our own version of verbose, below.
        TailableProc.__init__(self, lnd_dir)
        self.executable = "lnd"
        self.lnd_dir = lnd_dir
        self.port = port
        self.cmd_prefix = []

        self.bitcoind = bitcoind
        self.opts = LND_CONFIG.copy()
        opts = {
            "lnddir": lnd_dir,
            "listen": "127.0.0.1:{}".format(port),
            "rpclisten": "127.0.0.1:{}".format(grpc_port),
            "bitcoind.rpcuser": BITCOIND_CONFIG["rpcuser"],
            "bitcoind.rpcpass": BITCOIND_CONFIG["rpcpassword"],
            "bitcoind.zmqpubrawblock": "tcp://127.0.0.1:{}".format(bitcoind.blockport),
            "bitcoind.zmqpubrawtx": "tcp://127.0.0.1:{}".format(bitcoind.txport),
        }

        for k, v in opts.items():
            self.opts[k] = v

        if not os.path.exists(Path(lnd_dir)):
            os.makedirs(Path(lnd_dir))

        self.prefix = "lnd-%d" % (node_id)

        # TODO: create a logfile
        # Log to stdout so we see it in failure cases, and log file for TailableProc.
        # self.opts['log-file'] = ['-', os.path.join(lightning_dir, "log")]
        # self.opts['log-prefix'] = self.prefix + ' '

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

        return self.cmd_prefix + [self.executable] + opts

    def start(self, stdin=None, wait_for_initialized=True, stderr_redir=True):
        self.opts["bitcoind.rpchost"] = "localhost:{}".format(self.bitcoind.rpcport)
        TailableProc.start(self, stdin, stderr_redir=stderr_redir)
        if wait_for_initialized:
            self.wait_for_log("Waiting for wallet encryption password")
        logging.info("LND started")

    def wait(self, timeout=TIMEOUT):
        """Wait for the daemon to stop for up to timeout seconds

        Returns the returncode of the process, None if the process did
        not return before the timeout triggers.
        """
        self.proc.wait(timeout)
        return self.proc.returncode


def parse_scid(scid):
    parts = scid.split("x")
    return (
        ((long(parts[0]) & 0x0000000000FFFFFF) << 40)
        & ((long(parts[1]) & 0x0000000000FFFFFF) << 16)
        & (long(parts[2]) & 0x000000000000FFFF)
    )


class LndNode(object):
    def __init__(
        self,
        node_id,
        lnd_dir,
        bitcoind,
        port,
        grpc_port,
        options=None,
    ):
        self.logger = logging.getLogger("LndNode")
        self.bitcoin = bitcoind
        self.lnd_dir = Path(lnd_dir)
        self.port = port
        self.grpc_port = grpc_port
        self.is_initialized = False
        self.macaroon = None

        # Assume successful exit
        self.rc = 0
        self.rpc = None

        self.daemon = LND(
            lnd_dir,
            bitcoind=bitcoind,
            port=port,
            node_id=node_id,
            grpc_port=self.grpc_port,
        )

        if options is not None:
            self.daemon.opts.update(options)

    def _create_grpc_rpc(self):
        with (self.lnd_dir / "tls.cert").open(mode="rb") as f:
            tls_cert = f.read()

        # Now the node will actually start up and use them, so we can
        # create the RPC instance.
        self.rpc = LndGrpc(
            host="127.0.0.1",
            port=self.grpc_port,
            tls_cert=tls_cert,
            macaroon=self.macaroon,
        )

    def is_synced_with_bitcoin(self):
        info = self.rpc.get_info()
        return info["synced_to_chain"]

    def start(self, wait_for_bitcoind_sync=True):
        self.daemon.start()
        self._create_grpc_rpc()

        if self.is_initialized:
            self.logger.info("unlocking LND wallet")
            self.rpc.unlock_wallet()
        else:
            self.logger.info("initializing LND wallet")
            mnemonic = self.rpc.gen_seed()["cipher_seed_mnemonic"]
            self.macaroon = self.rpc.init_wallet(mnemonic)["macaroon"]
            self.is_initialized = True

        self.daemon.wait_for_log("Chain backend is fully synced")
        info = self.rpc.get_info()
        self.info = {"id": info["identity_pubkey"]}

        if wait_for_bitcoind_sync:
            wait_for(lambda: self.is_synced_with_bitcoin())

    def stop(self, timeout=TIMEOUT):
        # Tell the daemon to stop
        try:
            # May fail if the process already died
            self.rpc.stop()
        except Exception:
            pass

        self.rc = self.daemon.wait(timeout)

        # If it did not stop be more insistent
        if self.rc is None:
            self.rc = self.daemon.stop()

        return self.rc

    def connect(self, remote_node):
        self.rpc.connect_peer(
            remote_node.info["id"], "127.0.0.1:{}".format(remote_node.port)
        )

    def is_connected(self, remote_node):
        return remote_node.info["id"] in [
            p["pub_key"] for p in self.rpc.list_peers()["peers"]
        ]

    def fund_wallet(self, amount, mine_block=True):
        addr = self.rpc.new_address("p2tr")["address"]
        txid = self.bitcoin.rpc.sendtoaddress(addr, amount / 10**8)
        if mine_block:
            self.bitcoin.generate_block(1)
            self.daemon.wait_for_log(
                "Marking unconfirmed transaction {} mined in block".format(txid)
            )
        return addr, txid

    def open_channel(
        self, remote_node, capacity=FUNDAMOUNT, confirm=True, wait_for_active=True
    ):
        addr, wallettxid = self.fund_wallet(10 * capacity)

        if not self.is_connected(remote_node):
            self.connect(remote_node)

        res = self.rpc.open_channel(remote_node.info["id"], capacity)
        channel_point = "{}:{}".format(res["txid"], res["outnum"])

        print()
        if confirm or wait_for_active:
            self.bitcoin.generate_block(1, wait_for_mempool=res["txid"])

        if wait_for_active:
            self.bitcoin.generate_block(5)

            def channel_is_active():
                channels = [
                    c
                    for c in self.rpc.list_channels(remote_node.info["id"])["channels"]
                    if c["channel_point"] == channel_point
                ]

                if len(channels) == 0:
                    return False

                return channels[0]["active"]

            wait_for(channel_is_active)

        return {"txid": res["txid"], "outnum": res["outnum"]}

    def create_invoice(self, amount_msat, description="desc", preimage=None, cltv=None):
        inv = self.rpc.add_invoice(
            amount_msat, description, preimage=preimage, cltv=cltv
        )
        return inv["payment_request"]

    def send_onchain(self, address, amount, confirm=0):
        txid = self.rpc.send_coins(address, amount)["txid"]
        if confirm > 0:
            self.bitcoin.generate_block(1, wait_for_mempool=txid)
        if confirm > 1:
            self.bitcoin.generate_block(confirm - 1)
        return txid

    def list_invoices(self, payment_hash=None):
        invoices = self.rpc.list_invoices()["invoices"]
        if payment_hash is not None:
            invoices = [i for i in invoices if i["r_hash"] == payment_hash]

        return [
            {
                "bolt11": i["payment_request"],
                "paid": i["state"] == "SETTLED",
                "payment_hash": i["r_hash"],
            }
            for i in invoices
        ]

    def list_utxos(self):
        outputs = self.rpc.list_unspent()["utxos"]
        return [
            {"txid": o["txid"], "outnum": o["outnum"], "amount": o["amount"]}
            for o in outputs
        ]


class _ClientCallDetails(
    collections.namedtuple(
        "_ClientCallDetails", ("method", "timeout", "metadata", "credentials")
    ),
    grpc.ClientCallDetails,
):
    pass


class AuthenticationInterceptor(grpc.UnaryUnaryClientInterceptor):
    def __init__(self, interceptor_function):
        self._fn = interceptor_function

    def intercept_unary_unary(self, continuation, client_call_details, request):
        new_details, new_request_iterator, postprocess = self._fn(
            client_call_details, iter((request,)), False, False
        )
        response = continuation(new_details, next(new_request_iterator))
        return postprocess(response) if postprocess else response


def add_authentication(header, value):
    def intercept_call(
        client_call_details, request_iterator, request_streaming, response_streaming
    ):
        metadata = []
        if client_call_details.metadata is not None:
            metadata = list(client_call_details.metadata)
        metadata.append(
            (
                header,
                value,
            )
        )
        client_call_details = _ClientCallDetails(
            client_call_details.method,
            client_call_details.timeout,
            metadata,
            client_call_details.credentials,
        )
        return client_call_details, request_iterator, None

    return AuthenticationInterceptor(intercept_call)


class LndGrpc(object):
    def __init__(self, host: str, port: int, tls_cert: bytes, macaroon=None):
        self.logger = logging.getLogger("LndGrpc")
        self.logger.debug(f"Connecting to grpc interface at {host}:{port}")
        self.credentials = grpc.ssl_channel_credentials(
            root_certificates=tls_cert,
        )
        self.channel = grpc.secure_channel(
            f"{host}:{port}",
            self.credentials,
            # options=(("grpc.ssl_target_name_override", "lnd"),),
        )
        self.macaroon = macaroon
        self.interceptor = None
        self.stub = None
        self.wallet_stub = None
        self.unlocker_stub = WalletUnlockerStub(self.channel)
        if self.macaroon is not None:
            authentication = add_authentication("macaroon", self.macaroon)
            self.interceptor = grpc.intercept_channel(self.channel, authentication)
            self.stub = LightningStub(self.interceptor)
            self.wallet_stub = WalletKitStub(self.interceptor)

    def add_invoice(self, amount_msat, description, preimage=None, cltv=None):
        r_preimage = None
        if preimage is not None:
            r_preimage = bytes.fromhex(preimage)
        payload = lightning_pb2.Invoice(
            value_msat=amount_msat,
            memo=description,
            r_preimage=r_preimage,
            cltv_expiry=cltv,
        )
        resp = self.stub.AddInvoice(payload)
        return {"payment_request": resp.payment_request}

    def gen_seed(self):
        payload = walletunlocker_pb2.GenSeedRequest()
        resp = self.unlocker_stub.GenSeed(payload)
        return {"cipher_seed_mnemonic": resp.cipher_seed_mnemonic}

    def init_wallet(self, mnemonic, stateless=True, password="super-secret-password"):
        payload = walletunlocker_pb2.InitWalletRequest(
            wallet_password=password.encode(encoding="utf-8"),
            cipher_seed_mnemonic=mnemonic,
            stateless_init=stateless,
        )
        resp = self.unlocker_stub.InitWallet(payload)
        self.macaroon = resp.admin_macaroon.hex()
        authentication = add_authentication("macaroon", self.macaroon)
        self.interceptor = grpc.intercept_channel(self.channel, authentication)
        self.stub = LightningStub(self.interceptor)
        self.wallet_stub = WalletKitStub(self.interceptor)
        return {"macaroon": self.macaroon}

    def unlock_wallet(self, password="super-secret-password"):
        payload = walletunlocker_pb2.UnlockWalletRequest(
            wallet_password=password.encode(encoding="utf-8")
        )
        self.unlocker_stub.UnlockWallet(payload)

    def get_info(self):
        payload = lightning_pb2.GetInfoRequest()
        resp = self.stub.GetInfo(payload)
        return {
            "identity_pubkey": resp.identity_pubkey,
            "synced_to_chain": resp.synced_to_chain,
        }

    def stop(self):
        payload = lightning_pb2.StopRequest()
        self.stub.StopDaemon(payload)

    def connect_peer(self, pubkey: str, host: str):
        payload = lightning_pb2.ConnectPeerRequest(
            addr=lightning_pb2.LightningAddress(pubkey=pubkey, host=host)
        )
        self.stub.ConnectPeer(payload)

    def list_peers(self):
        payload = lightning_pb2.ListPeersRequest()
        resp = self.stub.ListPeers(payload)
        peers = [{"pub_key": p.pub_key} for p in resp.peers]
        return {"peers": peers}

    def new_address(self, type: str):
        t = lightning_pb2.AddressType.TAPROOT_PUBKEY
        if type == "p2wkh":
            t = lightning_pb2.AddressType.WITNESS_PUBKEY_HASH
        elif type == "np2wkh":
            t = lightning_pb2.AddressType.NESTED_PUBKEY_HASH
        payload = lightning_pb2.NewAddressRequest(type=t)
        resp = self.stub.NewAddress(payload)
        return {"address": resp.address}

    def open_channel(self, node_pubkey: str, local_funding_amount: int):
        payload = lightning_pb2.OpenChannelRequest(
            node_pubkey_string=node_pubkey, local_funding_amount=local_funding_amount
        )
        resp = self.stub.OpenChannelSync(payload)
        reverse_bytes = resp.funding_txid_bytes[::-1]
        txid = reverse_bytes.hex()
        return {"txid": txid, "outnum": resp.output_index}

    def list_channels(self, peer=None):
        if peer is not None:
            peer = bytes.fromhex(peer)
        payload = lightning_pb2.ListChannelsRequest(peer=peer)
        resp = self.stub.ListChannels(payload)
        return {
            "channels": [
                {"channel_point": c.channel_point, "active": c.active}
                for c in resp.channels
            ]
        }

    def list_unspent(self):
        payload = walletkit_pb2.ListUnspentRequest()
        resp = self.wallet_stub.ListUnspent(payload)
        return {
            "utxos": [
                {
                    "txid": u.outpoint.txid_str,
                    "outnum": u.outpoint.output_index,
                    "amount": u.amount_sat,
                }
                for u in resp.utxos
            ]
        }


class LndNodeFactory(object):
    """A factory to setup and start `lnd` daemons."""

    def __init__(self, bitcoind, directory, cltv_delta):
        self.next_id = 1
        self.nodes = []
        self.reserved_ports = []
        self.bitcoind = bitcoind
        self.directory = directory
        self.lock = threading.Lock()
        self.cltv_delta = cltv_delta

    def get_node_id(self):
        """Generate a unique numeric ID for a lightning node"""
        with self.lock:
            node_id = self.next_id
            self.next_id += 1
            return node_id

    def get_node(
        self,
        node_id=None,
        start=True,
        cleandir=True,
        wait_for_bitcoind_sync=True,
    ):
        node_id = self.get_node_id() if not node_id else node_id
        port = reserve_unused_port()
        grpc_port = reserve_unused_port()

        lnd_dir = os.path.join(self.directory, "lnd-{}/".format(node_id))

        if cleandir and os.path.exists(lnd_dir):
            shutil.rmtree(lnd_dir)

        options = {"bitcoin.timelockdelta": self.cltv_delta}
        node = LndNode(
            node_id,
            lnd_dir,
            self.bitcoind,
            port=port,
            grpc_port=grpc_port,
            options=options,
        )

        self.nodes.append(node)
        self.reserved_ports.append(port)
        self.reserved_ports.append(grpc_port)

        if start:
            try:
                node.start(wait_for_bitcoind_sync)
            except Exception:
                node.daemon.stop()
                raise
        return node

    def killall(self):
        for i in range(len(self.nodes)):
            try:
                self.nodes[i].stop()
            except Exception:
                pass

        for p in self.reserved_ports:
            drop_unused_port(p)

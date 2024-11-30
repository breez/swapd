from pyln.testing.utils import (
    LightningNode,
    drop_unused_port,
    reserve_unused_port,
    wait_for,
)

import random
import string

FUNDAMOUNT = 10**6


class ClnNode:
    def __init__(self, node, bitcoindproxy, port, grpc_port):
        self.bitcoin = bitcoindproxy
        self.port = port
        self.grpc_port = grpc_port
        self.node = node
        self.info = {}

    def start(self):
        self.info = {"id": self.node.info["id"]}

    def stop(self, timeout=10):
        return self.node.stop(timeout)

    def connect(self, remote_node):
        self.node.rpc.connect(remote_node.info["id"], "127.0.0.1", remote_node.port)

    def is_connected(self, remote_node):
        return remote_node.info["id"] in [
            p["id"] for p in self.node.rpc.listpeers()["peers"]
        ]

    def fund_wallet(self, amount):
        return self.node.fundwallet(10 * amount)

    def open_channel(
        self, remote_node, capacity=FUNDAMOUNT, confirm=True, wait_for_active=True
    ):
        addr, wallettxid = self.fund_wallet(10 * capacity)

        if not self.is_connected(remote_node):
            self.connect(remote_node)

        res = self.node.rpc.fundchannel(remote_node.info["id"], capacity)

        if confirm or wait_for_active:
            self.bitcoin.generate_block(1, wait_for_mempool=res["txid"])

        if wait_for_active:
            self.bitcoin.generate_block(5)
            wait_for(
                lambda: all(
                    channel["state"] == "CHANNELD_NORMAL"
                    for channel in self.node.rpc.listpeerchannels()["channels"]
                )
            )

            wait_for(
                lambda: [
                    "alias" in e
                    for e in self.node.rpc.listnodes(remote_node.info["id"])["nodes"]
                ]
                == [True]
            )

        return {"txid": res["txid"], "outnum": res["outnum"]}

    def create_invoice(self, amount_msat, description="desc", preimage=None):
        label = "".join(
            random.choice(string.ascii_letters + string.digits) for _ in range(20)
        )
        inv = self.node.rpc.invoice(amount_msat, label, description, preimage=preimage)
        return inv["bolt11"]

    def send_onchain(self, address, amount, confirm=0):
        txid = self.node.rpc.withdraw(address, amount)["txid"]
        if confirm > 0:
            self.bitcoin.generate_block(1, wait_for_mempool=txid)
        if confirm > 1:
            self.bitcoin.generate_block(confirm - 1)
        return txid

    def list_invoices(self, payment_hash=None):
        invoices = self.node.rpc.listinvoices(payment_hash=payment_hash)["invoices"]
        return [
            {
                "bolt11": i["bolt11"],
                "paid": i["status"] == "paid",
                "payment_hash": i["payment_hash"],
            }
            for i in invoices
        ]

    def list_utxos(self):
        outputs = self.node.rpc.listfunds()["outputs"]
        return [
            {
                "txid": o["txid"],
                "outnum": o["output"],
                "amount": o["amount_msat"] / 1000,
            }
            for o in outputs
        ]


class ClnNodeFactory(object):
    """A factory to setup and start wrapped `lightningd` daemons."""

    def __init__(self, node_factory):
        self.reserved_ports = []
        self.node_factory = node_factory

    def get_node(self):
        cln_grpc_port = reserve_unused_port()
        self.reserved_ports.append(cln_grpc_port)
        node = self.node_factory.get_node(options={"grpc-port": cln_grpc_port})
        cln_node = ClnNode(node, node.bitcoin, node.port, cln_grpc_port)
        cln_node.start()
        return cln_node

    def killall(self):
        for reserved_port in self.reserved_ports:
            drop_unused_port(reserved_port)

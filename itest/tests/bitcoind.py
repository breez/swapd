from pyln.testing.utils import (
    TIMEOUT,
    drop_unused_port,
    reserve_unused_port,
    write_config,
    SimpleBitcoinProxy,
    BitcoinRpcProxy,
    wait_for,
    BITCOIND_CONFIG,
    TailableProc,
)

import os
import logging


class BitcoinD(TailableProc):

    def __init__(
        self,
        bitcoin_dir="/tmp/bitcoind-test",
        rpcport=None,
        blockport=None,
        txport=None,
    ):
        TailableProc.__init__(self, bitcoin_dir, verbose=True)

        self.reserved_ports = []
        if rpcport is None:
            p = reserve_unused_port()
            self.reserved_ports.append(p)
            rpcport = p
        self.rpcport = rpcport

        if blockport is None:
            p = reserve_unused_port()
            self.reserved_ports.append(p)
            blockport = p
        self.blockport = blockport

        if txport is None:
            p = reserve_unused_port()
            self.reserved_ports.append(p)
            txport = p
        self.txport = txport

        self.bitcoin_dir = bitcoin_dir
        self.prefix = "bitcoind"

        regtestdir = os.path.join(bitcoin_dir, "regtest")
        if not os.path.exists(regtestdir):
            os.makedirs(regtestdir)

        self.cmd_line = [
            "bitcoind",
            "-regtest",
            "-datadir={}".format(bitcoin_dir),
            "-printtoconsole",
            "-server",
            "-logtimestamps",
            "-nolisten",
            "-txindex",
            "-addresstype=bech32",
            "-debug=mempool",
            "-debug=mempoolrej",
            "-debug=rpc",
            "-debug=validation",
            "-rpcthreads=20",
            "-fallbackfee=0.00000253",
            "-zmqpubrawblock=tcp://127.0.0.1:{}".format(blockport),
            "-zmqpubrawtx=tcp://127.0.0.1:{}".format(txport),
            "-rpcport={}".format(rpcport),
            "-rpcuser={}".format(BITCOIND_CONFIG["rpcuser"]),
            "-rpcpassword={}".format(BITCOIND_CONFIG["rpcpassword"]),
        ]
        # For up to and including 0.16.1, this needs to be in main section.
        BITCOIND_CONFIG["rpcport"] = rpcport
        self.conf_file = os.path.join(bitcoin_dir, "random_name.txt")
        write_config(self.conf_file, BITCOIND_CONFIG)
        self.rpc = SimpleBitcoinProxy(btc_conf_file=self.conf_file)
        self.proxies = []

    def __del__(self):
        for p in self.reserved_ports:
            drop_unused_port(p)

    def start(self):
        TailableProc.start(self)
        self.wait_for_log("Done loading", timeout=TIMEOUT)

        logging.info("BitcoinD started")
        try:
            self.rpc.createwallet("swapd-tests")
        except JSONRPCError:
            self.rpc.loadwallet("swapd-tests")

    def stop(self):
        for p in self.proxies:
            p.stop()
        self.rpc.stop()
        return TailableProc.stop(self)

    def get_proxy(self):
        proxy = BitcoinRpcProxy(self)
        self.proxies.append(proxy)
        proxy.start()
        return proxy

    # wait_for_mempool can be used to wait for the mempool before generating blocks:
    # True := wait for at least 1 transation
    # int > 0 := wait for at least N transactions
    # 'tx_id' := wait for one transaction id given as a string
    # ['tx_id1', 'tx_id2'] := wait until all of the specified transaction IDs
    def generate_block(
        self, numblocks=1, wait_for_mempool=0, to_addr=None, needfeerate=None
    ):
        if wait_for_mempool:
            if isinstance(wait_for_mempool, str):
                wait_for_mempool = [wait_for_mempool]
            if isinstance(wait_for_mempool, list):
                wait_for(
                    lambda: all(
                        txid in self.rpc.getrawmempool() for txid in wait_for_mempool
                    )
                )
            else:
                wait_for(lambda: len(self.rpc.getrawmempool()) >= wait_for_mempool)

        mempool = self.rpc.getrawmempool(True)
        logging.debug(
            "Generating {numblocks}, confirming {lenmempool} transactions: {mempool}".format(
                numblocks=numblocks,
                mempool=mempool,
                lenmempool=len(mempool),
            )
        )

        # As of 0.16, generate() is removed; use generatetoaddress.
        if to_addr is None:
            to_addr = self.rpc.getnewaddress()

        # We assume all-or-nothing.
        if needfeerate is not None:
            assert numblocks == 1
            # If any tx including ancestors is above the given feerate, mine all.
            for txid, details in mempool.items():
                feerate = (
                    float(details["fees"]["ancestor"])
                    * 100_000_000
                    / (float(details["ancestorsize"]) * 4 / 1000)
                )
                if feerate >= needfeerate:
                    return self.rpc.generatetoaddress(numblocks, to_addr)
                else:
                    print(f"Feerate {feerate} for {txid} below {needfeerate}")

            # Otherwise, mine none.
            return self.rpc.generateblock(to_addr, [])

        return self.rpc.generatetoaddress(numblocks, to_addr)

    def simple_reorg(self, height, shift=0):
        """
        Reorganize chain by creating a fork at height=[height] and re-mine all mempool
        transactions into [height + shift], where shift >= 0. Returns hashes of generated
        blocks.

        Note that tx's that become invalid at [height] (because coin maturity, locktime
        etc.) are removed from mempool. The length of the new chain will be original + 1
        OR original + [shift], whichever is larger.

        For example: to push tx's backward from height h1 to h2 < h1, use [height]=h2.

        Or to change the txindex of tx's at height h1:
        1. A block at height h2 < h1 should contain a non-coinbase tx that can be pulled
           forward to h1.
        2. Set [height]=h2 and [shift]= h1-h2
        """
        hashes = []
        fee_delta = 1000000
        orig_len = self.rpc.getblockcount()
        old_hash = self.rpc.getblockhash(height)
        final_len = height + shift if height + shift > orig_len else 1 + orig_len
        # TODO: raise error for insane args?

        self.rpc.invalidateblock(old_hash)
        self.wait_for_log(
            r"InvalidChainFound: invalid block=.*  height={}".format(height)
        )
        memp = self.rpc.getrawmempool()

        if shift == 0:
            hashes += self.generate_block(1 + final_len - height)
        else:
            for txid in memp:
                # lower priority (to effective feerate=0) so they are not mined
                self.rpc.prioritisetransaction(txid, None, -fee_delta)
            hashes += self.generate_block(shift)

            for txid in memp:
                # restore priority so they are mined
                self.rpc.prioritisetransaction(txid, None, fee_delta)
            hashes += self.generate_block(1 + final_len - (height + shift))
        self.wait_for_log(r"UpdateTip: new best=.* height={}".format(final_len))
        return hashes

    def getnewaddress(self):
        return self.rpc.getnewaddress()

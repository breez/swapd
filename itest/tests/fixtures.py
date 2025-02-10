from pyln.testing.fixtures import (
    directory,
    test_name,
    node_factory as pyln_node_factory,
    teardown_checks,
)
from bitcoind import BitcoinD
from cln import ClnNode, ClnNodeFactory
from lnd import LndNode, LndNodeFactory
from swapd import *
from postgres import PostgresContainerFactory
import pytest
import re
import time
import docker


@pytest.fixture
def bitcoind(directory, teardown_checks):
    bitcoind = BitcoinD(bitcoin_dir=directory)

    try:
        bitcoind.start()
    except Exception:
        bitcoind.stop()
        raise

    info = bitcoind.rpc.getnetworkinfo()

    # FIXME: include liquid-regtest in this check after elementsd has been
    # updated
    if info["version"] < 200100 and env("TEST_NETWORK") != "liquid-regtest":
        bitcoind.rpc.stop()
        raise ValueError(
            "bitcoind is too old. At least version 20100 (v0.20.1)"
            " is needed, current version is {}".format(info["version"])
        )
    elif info["version"] < 160000:
        bitcoind.rpc.stop()
        raise ValueError(
            "elementsd is too old. At least version 160000 (v0.16.0)"
            " is needed, current version is {}".format(info["version"])
        )

    info = bitcoind.rpc.getblockchaininfo()
    # Make sure we have some spendable funds
    if info["blocks"] < 101:
        bitcoind.generate_block(101 - info["blocks"])
    elif bitcoind.rpc.getwalletinfo()["balance"] < 1:
        logging.debug("Insufficient balance, generating 1 block")
        bitcoind.generate_block(1)

    yield bitcoind

    try:
        bitcoind.stop()
    except Exception:
        bitcoind.proc.kill()
    bitcoind.proc.wait()


@pytest.fixture
def whatthefee():
    wtf = WhatTheFee()
    wtf.start()

    yield wtf

    wtf.stop()


@pytest.fixture
def lock_time():
    return 50


@pytest.fixture
def min_claim_blocks():
    return 5


@pytest.fixture
def min_viable_cltv():
    return 8


# NOTE: cltv_delta should be higher than min_viable_cltv for the cltv tests to work
@pytest.fixture
def cltv_delta():
    return 18


def get_crash_log(swapd):
    if swapd.may_fail:
        return None, None
    try:
        crashlog = os.path.join(node.daemon.process_dir, "crash.log")
        with open(crashlog, "r") as f:
            return f.readlines(), crashlog
    except Exception:
        return None, None


def print_crash_log(swapd):
    errors, fname = get_crash_log(swapd)
    if errors:
        print("-" * 10, "{} (last 50 lines)".format(fname), "-" * 10)
        print("".join(errors[-50:]))
        print("-" * 80)
    return 1 if errors else 0


def get_err_log(swapd):
    for error_file in os.listdir(swapd.daemon.process_dir):
        if not re.fullmatch(r"errlog", error_file):
            continue
        with open(os.path.join(swapd.daemon.process_dir, error_file), "r") as f:
            errors = f.read().strip()
            if errors:
                return errors, error_file
    return None, None


def print_err_log(swapd):
    errors, fname = get_err_log(swapd)
    if errors:
        print(
            "-" * 31,
            "stderr of swapd {} captured in {} file".format(swapd.daemon.prefix, fname),
            "-" * 32,
        )
        print(errors)
        print("-" * 80)
    return 1 if errors else 0


@pytest.fixture
def postgres_factory(test_name, teardown_checks):
    pf = PostgresContainerFactory(test_name)
    yield pf
    errs = pf.killall()
    for e in errs:
        teardown_checks.add_error(e)


@pytest.fixture
def cln_factory(directory, bitcoind, cltv_delta):
    nf = ClnNodeFactory(bitcoind, directory, cltv_delta)
    yield nf
    nf.killall()


@pytest.fixture
def lnd_factory(directory, bitcoind, cltv_delta):
    nf = LndNodeFactory(bitcoind, directory, cltv_delta)
    yield nf
    nf.killall()


@pytest.fixture
def cln_options():
    nf = ClnOptionsProvider()
    yield nf


@pytest.fixture
def lnd_options(directory, bitcoind):
    nf = LndOptionsProvider()
    yield nf


@pytest.fixture()
def node_factory(cln_factory):
    return cln_factory


@pytest.fixture()
def options_provider(request):
    return request.param


@pytest.fixture()
def swapd_factory(
    request,
    directory,
    test_name,
    bitcoind,
    teardown_checks,
    postgres_factory,
    whatthefee,
    cln_factory,
    cln_options,
    lnd_factory,
    lnd_options,
    lock_time,
    min_claim_blocks,
    min_viable_cltv,
):
    node_factory = cln_factory
    options_provider = cln_options
    if request.param is not None and request.param == "lnd":
        node_factory = lnd_factory
        options_provider = lnd_options

    sf = SwapdFactory(
        test_name,
        bitcoind,
        whatthefee,
        directory=directory,
        node_factory=node_factory,
        options_provider=options_provider,
        postgres_factory=postgres_factory,
        lock_time=lock_time,
        min_claim_blocks=min_claim_blocks,
        min_viable_cltv=min_viable_cltv,
    )

    yield sf
    ok, errs = sf.killall([not s.may_fail for s in sf.instances])

    for e in errs:
        teardown_checks.add_error(e)

    for n in sf.instances:
        n.daemon.logs_catchup()

    def map_swapd_error(instances, f, msg):
        ret = False
        for n in instances:
            if n and f(n):
                ret = True
                teardown_checks.add_node_error(n, msg.format(n=n))
        return ret

    map_swapd_error(sf.instances, print_crash_log, "had crash.log files")
    map_swapd_error(
        sf.instances,
        lambda s: s.rc != 0 and s.rc is not None and not s.may_fail,
        "Swapd exited with return code {n.rc}",
    )
    if not ok:
        map_swapd_error(
            sf.instances,
            print_err_log,
            "some node failed unexpected, non-empty errlog file",
        )

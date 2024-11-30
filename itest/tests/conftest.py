import pytest
from fixtures import *


def pytest_addoption(parser):
    parser.addoption(
        "--node",
        action="append",
        default=[],
        help="List of node types to test. Options are 'cln' or 'lnd'",
    )


def pytest_generate_tests(metafunc):
    nodes = ["cln", "lnd"]
    if "swapd_factory" in metafunc.fixturenames:
        configured_nodes = metafunc.config.getoption("node")
        assert all(
            n in ["cln", "lnd"] for n in configured_nodes
        ), "node must be 'cln' or 'lnd'"
        if len(configured_nodes) > 0:
            nodes = configured_nodes

    metafunc.parametrize("swapd_factory", nodes, indirect=True)

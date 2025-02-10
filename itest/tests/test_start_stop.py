from helpers import *


def test_start_stop(swapd_factory):
    swapd = swapd_factory.get_swapd()
    swapd.start()
    rc = swapd.stop()
    assert rc == 0

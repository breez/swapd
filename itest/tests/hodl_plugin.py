#!/usr/bin/env python3
from pyln.client import Plugin
import threading
import time

plugin = Plugin()
lock = threading.Lock()
event_listeners = []
resolutions = []


@plugin.method("resolve")
def resolve(plugin, index=-1, result={"result": "continue"}):
    """Resolves all htlcs currently pending."""
    plugin.log("resolve called")
    with lock:
        if index == -1:
            for i in range(len(event_listeners)):
                resolutions[i] = result
                event_listeners[i].set()
        else:
            resolutions[index] = result
            event_listeners[index].set()

    return {}


@plugin.init()
def init(options, configuration, plugin, **kwargs):
    plugin.log("Plugin hodl_plugin.py initialized")


@plugin.async_hook("htlc_accepted")
def on_htlc_accepted(onion, htlc, plugin, request, **kwargs):
    plugin.log("on_htlc_accepted called")
    resolve_called = threading.Event()
    with lock:
        index = len(event_listeners)
        event_listeners.append(resolve_called)
        resolutions.append({"result": "continue"})

    t = threading.Thread(
        target=hodl_htlc, args=(plugin, request, resolve_called, index)
    )
    t.start()


def hodl_htlc(plugin, request, resolve_called, index):
    plugin.log("hodl_htlc called")
    resolve_called.wait()
    request.set_result(resolutions[index])


plugin.run()

import pytest


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
def swapd_factory(
    request,
    directory,
    test_name,
    bitcoind,
    executor,
    db_provider,
    teardown_checks,
    node_cls,
    jsonschemas,
):
    nf = SwapdFactory(
        request,
        test_name,
        bitcoind,
        executor,
        directory=directory,
        db_provider=db_provider,
        node_cls=node_cls,
        jsonschemas=jsonschemas,
    )

    yield sf
    ok, errs = sf.killall([not s.may_fail for s in sf.instances])

    for e in errs:
        teardown_checks.add_error(e)

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
        lambda s: s.rc != 0 and not s.may_fail,
        "Swapd exited with return code {n.rc}",
    )
    if not ok:
        map_swapd_error(
            sf.instances,
            print_err_log,
            "some node failed unexpected, non-empty errlog file",
        )
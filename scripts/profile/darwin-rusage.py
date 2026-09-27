#!/usr/bin/env python3
"""proc_pid_rusage(2) for one pid, as `user_ns system_ns interrupt_wkups idle_wkups rss_bytes`.

Darwin has no /proc, and `/usr/bin/time -l` only reports a process it starts
itself. `proc_pid_rusage` is the counter macOS's own battery accounting reads,
it is available for any process of the same user, and its interrupt-wakeup
count is the closest Darwin analogue of the context-switch total the Linux path
sums out of /proc/<pid>/task/*/status.

RUSAGE_INFO_V0 is enough, and is the flavour every macOS since 10.9 accepts.
"""

import ctypes
import ctypes.util
import sys


class RUsageInfoV0(ctypes.Structure):
    _fields_ = [
        ("ri_uuid", ctypes.c_uint8 * 16),
        ("ri_user_time", ctypes.c_uint64),
        ("ri_system_time", ctypes.c_uint64),
        ("ri_pkg_idle_wkups", ctypes.c_uint64),
        ("ri_interrupt_wkups", ctypes.c_uint64),
        ("ri_pageins", ctypes.c_uint64),
        ("ri_wired_size", ctypes.c_uint64),
        ("ri_resident_size", ctypes.c_uint64),
        ("ri_phys_footprint", ctypes.c_uint64),
        ("ri_proc_start_abstime", ctypes.c_uint64),
        ("ri_proc_exit_abstime", ctypes.c_uint64),
    ]


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: darwin-rusage.py <pid>", file=sys.stderr)
        return 2
    pid = int(sys.argv[1])

    libc = ctypes.CDLL(ctypes.util.find_library("System") or "libSystem.dylib")
    info = RUsageInfoV0()
    rc = libc.proc_pid_rusage(ctypes.c_int(pid), ctypes.c_int(0), ctypes.byref(info))
    if rc != 0:
        print(f"proc_pid_rusage({pid}) failed: rc={rc}", file=sys.stderr)
        return 1

    print(
        f"{info.ri_user_time} {info.ri_system_time} "
        f"{info.ri_interrupt_wkups} {info.ri_pkg_idle_wkups} {info.ri_resident_size}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

"""Read the runtime IPC version from its authoritative Rust contract."""

from pathlib import Path
import re


_source = Path(__file__).resolve().parents[2] / "crates/copypaste-ipc/src/lib.rs"
_versions = re.findall(r"^pub const PROTOCOL_VERSION:\s*u32\s*=\s*([1-9][0-9]*)\s*;", _source.read_text(), re.MULTILINE)
if len(_versions) != 1:
    raise ValueError("The runtime IPC contract must define one protocol version")
PROTOCOL_VERSION = int(_versions[0])

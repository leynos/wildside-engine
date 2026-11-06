"""Contract test for the serial nextest group around nested compiler runs.

A test that starts a nested ``cargo`` compile (here, ``cargo clippy`` over the
fixture crate) must run in a nextest test group limited to one thread, with a
slow-timeout of its own that fits inside the profile's ``global-timeout``. The
configuration is parsed, never matched as text, so reformatting cannot break it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROFILE = "default"


def _config() -> dict:
    """Return the parsed nextest configuration."""
    return tomllib.loads((ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8"))


def _seconds(period: str) -> int:
    """Convert a nextest duration such as ``120s`` or ``10m`` to seconds."""
    match = re.fullmatch(r"(\d+)([smh])", period)
    assert match, f"unrecognized nextest duration: {period!r}"
    return int(match.group(1)) * {"s": 1, "m": 60, "h": 3600}[match.group(2)]


def _serial_overrides() -> list[dict]:
    """Return the profile's overrides that place tests in a one-thread group."""
    config = _config()
    serial = {
        name for name, group in config.get("test-groups", {}).items() if group.get("max-threads") == 1
    }
    overrides = config["profile"][PROFILE].get("overrides", [])
    return [o for o in overrides if o.get("test-group") in serial]


def test_a_serial_group_holds_the_nested_clippy_probe() -> None:
    """The probe binary is placed in a one-thread group, and the binary exists."""
    binaries = {
        name
        for override in _serial_overrides()
        for name in re.findall(r"binary\((\w+)\)", override.get("filter", ""))
    }
    assert binaries, "no override places a binary in a one-thread test group"
    for name in binaries:
        assert (ROOT / "tests" / f"{name}.rs").is_file(), f"{name} is not a top-level test binary"
    assert "clippy_env_policy_ui_tests" in binaries


def test_the_group_timeout_fits_inside_the_global_timeout() -> None:
    """Each serial override's total allowance stays below the global timeout."""
    global_timeout = _seconds(_config()["profile"][PROFILE]["global-timeout"])
    for override in _serial_overrides():
        slow = override["slow-timeout"]
        total = _seconds(slow["period"]) * slow.get("terminate-after", 1)
        assert total < global_timeout, f"{override['filter']} may outlast the global timeout"

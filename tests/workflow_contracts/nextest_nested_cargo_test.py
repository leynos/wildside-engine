"""Contract test for the serial nextest group around nested compiler runs.

A test that starts a nested ``cargo`` compile (here, ``cargo clippy`` over the
fixture crate) must run in a nextest test group limited to one thread, with a
slow-timeout of its own that fits inside the profile's ``global-timeout``. The
configuration is parsed, never matched as text, so reformatting cannot break it.
The tests do not spawn nextest or cargo.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
PROFILE = "default"
PROBE = "clippy_env_policy_ui_tests"
DEFAULT_SLOW_PERIOD_SECONDS = 60
UNIT_SECONDS = {"s": 1, "m": 60, "h": 3600}
# A filter that only includes binaries: one or more `binary(name)` terms joined by `|`.
INCLUDING_FILTER = re.compile(r"\s*binary\(\w+\)(\s*\|\s*binary\(\w+\))*\s*")


@pytest.fixture(name="config")
def config_fixture() -> dict:
    """Return the parsed nextest configuration."""
    return tomllib.loads((ROOT / ".config" / "nextest.toml").read_text(encoding="utf-8"))


def _seconds(period: str) -> int:
    """Convert a nextest duration such as ``120s`` or ``10m`` to seconds."""
    match = re.fullmatch(r"(\d+)([smh])", period)
    assert match, f"unrecognized nextest duration: {period!r}"
    return int(match.group(1)) * UNIT_SECONDS[match.group(2)]


def _serial_overrides(config: dict) -> list[dict]:
    """Return the profile's overrides that place tests in a one-thread group."""
    serial = {
        name for name, group in config.get("test-groups", {}).items() if group.get("max-threads") == 1
    }
    overrides = config["profile"][PROFILE].get("overrides", [])
    return [o for o in overrides if o.get("test-group") in serial]


def _included_binaries(filter_expression: str) -> set[str]:
    """Return the binaries an including-only filter names; empty for any other shape."""
    if not INCLUDING_FILTER.fullmatch(filter_expression):
        return set()
    return set(re.findall(r"binary\((\w+)\)", filter_expression))


@pytest.mark.parametrize(
    ("expression", "expected"),
    [
        ("binary(a)", {"a"}),
        ("binary(a) | binary(b)", {"a", "b"}),
        ("not binary(a)", set()),
        ("!binary(a)", set()),
        ("binary(a) and none()", set()),
        ("binary(a) & binary(b)", set()),
    ],
)
def test_only_including_filters_count(expression: str, expected: set[str]) -> None:
    """A negated or intersecting filter does not count as placing the binary."""
    assert _included_binaries(expression) == expected


def test_a_serial_group_holds_the_nested_clippy_probe(config: dict) -> None:
    """The probe binary is included in a one-thread group, and the binary exists."""
    binaries = {
        name
        for override in _serial_overrides(config)
        for name in _included_binaries(override.get("filter", ""))
    }
    for name in binaries:
        assert (ROOT / "tests" / f"{name}.rs").is_file(), f"{name} is not a top-level test binary"
    assert PROBE in binaries


def test_the_group_allowance_exceeds_the_default_and_fits_the_global_timeout(config: dict) -> None:
    """Each serial override's total allowance is above the default and below the global timeout."""
    global_timeout = _seconds(config["profile"][PROFILE]["global-timeout"])
    overrides = _serial_overrides(config)
    assert overrides, "no override places tests in a one-thread group"
    for override in overrides:
        slow = override["slow-timeout"]
        assert "terminate-after" in slow, f"{override['filter']} never terminates a slow test"
        total = _seconds(slow["period"]) * slow["terminate-after"]
        assert total > DEFAULT_SLOW_PERIOD_SECONDS, f"{override['filter']} adds no allowance"
        assert total < global_timeout, f"{override['filter']} may outlast the global timeout"

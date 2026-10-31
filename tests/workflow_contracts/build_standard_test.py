"""Contract tests for the Rust build standard.

The Concordat ``rust-build-defaults`` rule needs the parallel ``rustc`` frontend
in every ``rustflags`` source, ``mold`` in the Linux source only, the gate
recipes that assign ``RUSTFLAGS`` to restate both, and a runner that has the
linker. Cranelift is not adopted (see "Cranelift is not the development
backend" in ``docs/developers-guide.md``), so these tests also keep the
configuration free of it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest
import tomllib
import yaml

ROOT = Path(__file__).resolve().parents[2]
THREADS = "-Zthreads=8"
MOLD = "-Clink-arg=-fuse-ld=mold"
LINUX_TABLE = 'cfg(target_os = "linux")'
GATE_RECIPES = ("test", "test-benches", "typecheck")


def _config() -> dict[str, object]:
    return tomllib.loads((ROOT / ".cargo" / "config.toml").read_text("utf-8"))


def test_every_rustflags_source_carries_the_parallel_frontend() -> None:
    """``[build]`` and the Linux target table both enable ``-Zthreads=8``."""
    config = _config()
    assert THREADS in config["build"]["rustflags"]
    assert THREADS in config["target"][LINUX_TABLE]["rustflags"]


def test_mold_is_configured_for_linux_only() -> None:
    """The linker flag is in the Linux table and in no other source."""
    config = _config()
    assert MOLD in config["target"][LINUX_TABLE]["rustflags"]
    assert MOLD not in config["build"]["rustflags"]
    others = [name for name in config["target"] if name != LINUX_TABLE]
    assert not others, f"unexpected target tables {others}"


def test_cranelift_stays_out_of_the_configuration() -> None:
    """The recorded exception holds while the configuration omits the backend."""
    config = _config()
    assert "codegen-backend" not in config.get("unstable", {})
    assert "profile" not in config, "no profile selects a backend"


def _recipe_commands(target: str, host_os: str) -> str:
    """Return the commands ``make -n`` prints for ``target`` on ``host_os``."""
    completed = subprocess.run(
        ["make", "-n", "-s", f"BUILD_HOST_OS={host_os}", target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return completed.stdout


@pytest.mark.parametrize("target", GATE_RECIPES)
def test_gate_recipes_restate_both_flags_on_linux(target: str) -> None:
    """A recipe that assigns ``RUSTFLAGS`` restates the frontend and mold."""
    commands = _recipe_commands(target, "Linux")
    assert "RUSTFLAGS=" in commands
    assert THREADS in commands
    assert MOLD in commands


@pytest.mark.parametrize("target", GATE_RECIPES)
def test_gate_recipes_leave_mold_off_other_platforms(target: str) -> None:
    """Off Linux the recipe keeps the frontend flag and drops the linker."""
    commands = _recipe_commands(target, "Darwin")
    assert THREADS in commands
    assert MOLD not in commands


def test_gate_recipes_keep_an_inherited_rustflags() -> None:
    """The recipe appends to an inherited value instead of replacing it."""
    commands = _recipe_commands("test", "Linux")
    assert re.search(r'RUSTFLAGS="\$\{RUSTFLAGS:\+\$RUSTFLAGS \}', commands)


def test_whitaker_keeps_its_own_flag_boundary() -> None:
    """The Whitaker driver does not inherit the development flags."""
    commands = _recipe_commands("lint", "Linux")
    whitaker = [line for line in commands.splitlines() if "whitaker" in line]
    assert whitaker, "the lint recipe must run Whitaker"
    assert all(THREADS not in line and MOLD not in line for line in whitaker)


def _workflow(name: str) -> dict[str, object]:
    return yaml.safe_load((ROOT / ".github" / "workflows" / name).read_text("utf-8"))


def _setup_rust_steps(job: dict[str, object]) -> list[dict[str, object]]:
    return [
        step
        for step in job["steps"]
        if "/actions/setup-rust@" in str(step.get("uses", ""))
    ]


@pytest.mark.parametrize("job_name", ["build", "feature-matrix"])
def test_ci_jobs_that_run_make_install_mold(job_name: str) -> None:
    """Each CI job that links through the Makefile installs ``mold``."""
    job = _workflow("ci.yml")["jobs"][job_name]
    steps = _setup_rust_steps(job)
    assert steps, f"{job_name} must run setup-rust"
    assert all(step["with"].get("install-mold") == "true" for step in steps)


def test_mutation_workflow_installs_mold() -> None:
    """cargo-mutants builds under the configuration, so it needs ``mold``."""
    job = _workflow("mutation-testing.yml")["jobs"]["mutation"]
    assert job["with"].get("install-mold") == "true"

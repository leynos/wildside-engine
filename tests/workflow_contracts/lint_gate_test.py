"""Contract tests for the CI lint gate's Clippy flags.

The Makefile defines ``CLIPPY_FLAGS ?= --workspace --all-targets
--all-features -- -D warnings``. The ``?=`` is deliberate, so a developer
can narrow the flags while iterating on one package, but a value exported
into make's environment overrides ``?=``. A ``CLIPPY_FLAGS`` entry in any
``env:`` mapping that reaches the lint step, whether declared at workflow,
job, or step level, would therefore silently drop targets or features from
the gate that blocks a merge while the Makefile still reads correctly.

``tests/clippy_env_policy_tests.rs`` asserts the Makefile default. These
tests assert that CI uses it, by walking every ``env:`` scope rather than
only the lint step's own line.

Mutation proof (run 2026-09-07). Each mutation failed only the test named
beside it: giving the lint step ``run: make lint CLIPPY_FLAGS=--workspace``
failed ``test_lint_step_runs_make_lint_bare``; adding
``env: {CLIPPY_FLAGS: --workspace}`` at workflow, job, and step level each
failed ``test_no_env_scope_overrides_the_clippy_flags``, and an earlier
draft that read only the step's own line survived all three.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest
import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"

#: The variable the Makefile leaves overridable and CI must not set.
CLIPPY_FLAGS = "CLIPPY_FLAGS"

#: The lint gate's invocation, with no arguments of its own.
BARE_LINT_COMMAND = "make lint"


@pytest.fixture(scope="module")
def workflow() -> dict[str, Any]:
    """Parse the CI workflow."""
    parsed = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    assert isinstance(parsed, dict), "ci.yml must parse to a mapping"
    return parsed


def _jobs(workflow: dict[str, Any]) -> dict[str, Any]:
    """Return the workflow's jobs mapping."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "ci.yml must declare a jobs mapping"
    return jobs


def _steps(job: Any) -> list[dict[str, Any]]:
    """Return a job's steps, or an empty list for a reusable-workflow call."""
    if not isinstance(job, dict):
        return []
    steps = job.get("steps")
    if not isinstance(steps, list):
        return []
    return [step for step in steps if isinstance(step, dict)]


def _env_scopes(workflow: dict[str, Any]) -> list[tuple[str, Any]]:
    """Return every env mapping in the workflow, each with a readable label."""
    scopes: list[tuple[str, Any]] = [("workflow", workflow.get("env"))]
    for job_name, job in _jobs(workflow).items():
        if isinstance(job, dict):
            scopes.append((f"job {job_name}", job.get("env")))
        for index, step in enumerate(_steps(job)):
            label = step.get("name") or step.get("uses") or f"step {index}"
            scopes.append((f"job {job_name}, step {label}", step.get("env")))
    return [(label, env) for label, env in scopes if env is not None]


def _lint_steps(workflow: dict[str, Any]) -> list[str]:
    """Return every run command in the workflow that invokes the lint gate."""
    commands: list[str] = []
    for job in _jobs(workflow).values():
        for step in _steps(job):
            run = step.get("run")
            if isinstance(run, str) and BARE_LINT_COMMAND in run:
                commands.append(run.strip())
    return commands


def test_lint_step_runs_make_lint_bare(workflow: dict[str, Any]) -> None:
    """The gate invokes the Make target without arguments of its own.

    A step that passed its own flags would bypass the Makefile default that
    `tests/clippy_env_policy_tests.rs` asserts.
    """
    commands = _lint_steps(workflow)
    assert commands, "ci.yml must run the lint gate"
    for command in commands:
        assert command == BARE_LINT_COMMAND, (
            f"the lint gate must run {BARE_LINT_COMMAND!r} bare, found {command!r}"
        )


def test_no_env_scope_overrides_the_clippy_flags(workflow: dict[str, Any]) -> None:
    """No env mapping sets CLIPPY_FLAGS at any level.

    An exported value overrides the Makefile's `?=`, so a workflow-, job-, or
    step-level entry would narrow the gate invisibly.
    """
    offenders = [
        f"{label} sets {CLIPPY_FLAGS}={env[CLIPPY_FLAGS]!r}"
        for label, env in _env_scopes(workflow)
        if isinstance(env, dict) and CLIPPY_FLAGS in env
    ]
    assert not offenders, (
        f"no CI env scope may set {CLIPPY_FLAGS}; found {offenders}"
    )

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

Asserting the command is not enough on its own. A gate can be neutralized
without its ``run`` value changing at all: ``if: false`` on the step or on
its job skips it, and so does any plausible-looking condition such as one
restricting the step to pushes. A ``needs`` prerequisite does it more
quietly still, because a skipped prerequisite skips its dependents and
GitHub reports a skipped job as successful. Removing the ``pull_request``
trigger, or narrowing it with branch, path, or activity-type filters,
takes the gate off the merge path for the pull requests the filter
excludes. These tests therefore require the command, the absence of any
condition or prerequisite on the step and its job, and an unfiltered
trigger.

No falsy spelling is enumerated. Requiring the absence of a condition
covers every value a condition could take, which matters because PyYAML
parses ``if: false`` to a boolean whose string form is ``False`` and an
expression to a string.

Mutation proof (run 2026-09-07). Each mutation failed only the test named
beside it:

- ``run: make lint CLIPPY_FLAGS=--workspace`` on the step, and wrapping the
  run as ``if false; then make lint; fi`` or burying it in a multiline run,
  each failed ``test_lint_step_runs_make_lint_bare``;
- ``env: {CLIPPY_FLAGS: --workspace}`` at workflow, job, and step level each
  failed ``test_no_env_scope_overrides_the_clippy_flags``; an earlier draft
  that read only the step's own line survived all three;
- ``if: false`` on the step, ``if: false`` on the job, a push-only condition
  on the step, and ``needs`` on the job named either as a string or as a
  list each failed ``test_nothing_conditions_away_the_lint_gate``. An empty
  ``needs: []`` is accepted, because it gates on nothing;
- removing the ``pull_request`` trigger, and narrowing it with ``branches``,
  ``paths``, or ``types``, each failed
  ``test_the_lint_gate_runs_on_pull_requests``.

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


#: Trigger filters that would exclude some pull requests from the gate.
TRIGGER_FILTERS = ("branches", "branches-ignore", "paths", "paths-ignore", "types")


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


def _lint_sites(workflow: dict[str, Any]) -> list[tuple[str, Any, dict[str, Any]]]:
    """Return each lint invocation with the job and step that carry it."""
    sites: list[tuple[str, Any, dict[str, Any]]] = []
    for job_name, job in _jobs(workflow).items():
        for step in _steps(job):
            run = step.get("run")
            if isinstance(run, str) and BARE_LINT_COMMAND in run:
                sites.append((job_name, job, step))
    return sites


def _triggers(workflow: dict[str, Any]) -> dict[str, Any]:
    """Return the ``on:`` mapping.

    YAML 1.1 reads a bare ``on`` key as the boolean true, so PyYAML stores
    the triggers under ``True`` rather than under the string.
    """
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "ci.yml must declare an on: mapping"
    return triggers


def test_lint_step_runs_make_lint_bare(workflow: dict[str, Any]) -> None:
    """The gate invokes the Make target without arguments of its own.

    A step that passed its own flags would bypass the Makefile default that
    `tests/clippy_env_policy_tests.rs` asserts.
    """
    sites = _lint_sites(workflow)
    assert sites, "ci.yml must run the lint gate"
    for _, _, step in sites:
        command = str(step["run"]).strip()
        assert command == BARE_LINT_COMMAND, (
            f"the lint gate must run {BARE_LINT_COMMAND!r} bare, found {command!r}"
        )


def test_nothing_conditions_away_the_lint_gate(workflow: dict[str, Any]) -> None:
    """Neither the lint step nor its job carries a condition or prerequisite.

    A condition skips the gate with the run command untouched, so asserting
    the command alone would certify a step that never executes. The absence
    of a condition is required rather than particular values, because a
    condition that looks plausible, such as one restricting the step to
    pushes, disables the merge gate just as completely as `if: false`.

    A `needs` prerequisite is the quieter form of the same thing: a skipped
    prerequisite skips its dependents, and a skipped job reports as
    successful. An empty `needs` is accepted because it waits on nothing.
    """
    sites = _lint_sites(workflow)
    assert sites, "ci.yml must run the lint gate"
    conditioned = [
        f"job {job_name} carries if: {job['if']!r}"
        for job_name, job, _ in sites
        if isinstance(job, dict) and "if" in job
    ] + [
        f"the lint step in job {job_name} carries if: {step['if']!r}"
        for job_name, _, step in sites
        if "if" in step
    ] + [
        f"job {job_name} carries needs: {job['needs']!r}"
        for job_name, job, _ in sites
        if isinstance(job, dict) and job.get("needs")
    ]
    assert not conditioned, f"nothing may condition the lint gate; found {conditioned}"


def test_the_lint_gate_runs_on_pull_requests(workflow: dict[str, Any]) -> None:
    """The workflow is triggered by every pull request, unfiltered.

    The lint gate blocks a merge only if it runs before one. A workflow
    restricted to pushes would leave every assertion above satisfied and the
    gate absent from the pull request. A branch, path, or activity-type
    filter does the same for the pull requests it excludes, which is worse
    for being selective: the gate still runs often enough to look present.
    """
    triggers = _triggers(workflow)
    assert "pull_request" in triggers, (
        f"ci.yml must be triggered by pull_request, found {sorted(map(str, triggers))}"
    )
    filters = triggers["pull_request"]
    if filters is None:
        return
    assert isinstance(filters, dict), (
        f"the pull_request trigger must be a mapping or empty, found {filters!r}"
    )
    narrowed = [key for key in TRIGGER_FILTERS if key in filters]
    assert not narrowed, (
        f"the pull_request trigger must not be narrowed; found {narrowed}"
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

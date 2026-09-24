"""Refusal cases for the coverage lanes: pull requests ratchet, main writes.

Each case changes one thing in the compliant fixture tree and asserts on
the one rule that must refuse it, so deleting that rule's clause fails
the case.
"""

from __future__ import annotations

import pytest

from codescene_contract.fixtures import (
    PULL_REQUEST_LANE,
    REPOSITORY,
    mutate,
    tree,
)
from codescene_contract.lanes import (
    publisher_lane_violations,
    pull_request_lane_violations,
    second_writer_violations,
)
from codescene_contract.loading import Document, load_workflow


def _documents(texts: dict[str, str]) -> dict[str, Document]:
    """Parse a tree of texts."""
    return {name: load_workflow(text) for name, text in texts.items()}


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("          with-ratchet: 'true'\n", ""),
        ("          publish-artefact: 'false'\n", ""),
        # GitHub passes `yes` and `no` to the action as strings.
        ("          with-ratchet: 'true'\n", "          with-ratchet: yes\n"),
        ("          publish-artefact: 'false'\n", "          publish-artefact: no\n"),
    ],
)
def test_a_pull_request_lane_ratchets_and_publishes_nothing(old: str, new: str) -> None:
    """A lane without the ratchet, or publishing its report, is refused."""
    documents = _documents(mutate("ci.yml", old, new))
    found = pull_request_lane_violations({"ci.yml": documents["ci.yml"]})
    assert found, found


@pytest.mark.parametrize(
    "guard",
    [
        "",
        "        if: always()\n",
        "        if: github.event_name == 'pull_request' || always()\n",
        "        if: ${{ !(github.event_name == 'pull_request') }}\n",
        "        if: github.event_name == 'pull_request' && ${{ true }}\n",
    ],
)
def test_a_push_lane_cannot_write_a_second_baseline(guard: str) -> None:
    """Coverage on a push outside the publisher is refused."""
    texts = mutate("ci.yml", "        if: github.event_name == 'pull_request'\n", guard)
    found = second_writer_violations(_documents(texts), "coverage-main.yml", REPOSITORY)
    assert found, found


def test_a_job_level_pull_request_guard_is_accepted() -> None:
    """A job guarded to pull requests never runs on a push, nor do its steps."""
    texts = mutate(
        "ci.yml",
        "    runs-on: ubuntu-latest\n",
        "    runs-on: ubuntu-latest\n    if: github.event_name == 'pull_request'\n",
    )
    texts["ci.yml"] = texts["ci.yml"].replace(
        "        if: github.event_name == 'pull_request'\n", ""
    )
    found = second_writer_violations(_documents(texts), "coverage-main.yml", REPOSITORY)
    assert not found, found


@pytest.mark.parametrize(
    ("guard", "expected"),
    [
        ("    if: github.event_name == 'pull_request'\n", False),
        ("", True),
    ],
)
def test_a_call_from_a_guarded_job_is_not_a_push_writer(
    guard: str, *, expected: bool
) -> None:
    """A callee reached only through a pull-request job never runs on a push."""
    caller = (
        "on: push\njobs:\n  call:\n" + guard + "    uses: ./.github/workflows/cov.yml\n"
    )
    callee = PULL_REQUEST_LANE.replace(
        "on:\n  push:\n    branches: [main]\n  pull_request:\n",
        "on:\n  workflow_call:\n",
    ).replace("        if: github.event_name == 'pull_request'\n", "")
    documents = _documents(tree(extra={"caller.yml": caller, "cov.yml": callee}))
    found = second_writer_violations(documents, "coverage-main.yml", REPOSITORY)
    assert bool(found) == expected, found


def test_a_differently_cased_action_is_still_a_second_writer() -> None:
    """GitHub resolves the owner without case, so the rule must too."""
    texts = mutate("ci.yml", "        if: github.event_name == 'pull_request'\n", "")
    texts["ci.yml"] = texts["ci.yml"].replace(
        "leynos/shared-actions", "Leynos/Shared-Actions"
    )
    found = second_writer_violations(_documents(texts), "coverage-main.yml", REPOSITORY)
    assert found, found


def test_a_push_lane_cannot_write_a_baseline_through_a_callee() -> None:
    """A push workflow's local callee runs on the push, so its coverage counts."""
    caller = "on: push\njobs:\n  call:\n    uses: ./.github/workflows/cov.yml\n"
    callee = PULL_REQUEST_LANE.replace(
        "on:\n  push:\n    branches: [main]\n  pull_request:\n",
        "on:\n  workflow_call:\n",
    ).replace("        if: github.event_name == 'pull_request'\n", "")
    documents = _documents(tree(extra={"caller.yml": caller, "cov.yml": callee}))
    found = second_writer_violations(documents, "coverage-main.yml", REPOSITORY)
    assert (
        "cov.yml: generate-coverage can run on a push; guard it to pull requests"
        in found
    ), found


@pytest.mark.parametrize(
    ("name", "old", "new"),
    [
        (
            "ci.yml",
            "          output-path: coverage.xml\n",
            "          output-path: other.xml\n",
        ),
        ("coverage-main.yml", "          with-ratchet: 'true'\n", ""),
        ("ci.yml", "generate-coverage@" + "a" * 40, "generate-coverage@" + "b" * 40),
    ],
)
def test_the_publisher_measures_what_each_lane_measures(
    name: str, old: str, new: str
) -> None:
    """A selection or pin differing from the publisher's is refused."""
    documents = _documents(mutate(name, old, new))
    closure = {"ci.yml": documents["ci.yml"]}
    found = publisher_lane_violations(documents["coverage-main.yml"], closure)
    assert found, found

"""The CV-005 contract over this repository's own workflows.

Main owns CodeScene: one push-to-main publisher uploads coverage and
writes the ratchet baseline, and nothing a pull request can start talks
to CodeScene or holds its credential. The rule tests beside this module
prove each clause refuses the shape it exists to refuse.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest

from codescene_contract.actions import read_actions
from codescene_contract.credential import (
    check_step_violations,
    token_scope_violations,
)
from codescene_contract.lanes import (
    publisher_lane_violations,
    pull_request_lane_violations,
    second_writer_violations,
)
from codescene_contract.loading import Document, read_workflows
from codescene_contract.publisher import find_publisher
from codescene_contract.publisher_rules import (
    concurrency_violations,
    condition_violations,
    permissions_violations,
    retired_checksum_violations,
    trigger_violations,
    upload_step_violations,
    wiring_violations,
)
from codescene_contract.reach import pull_request_closure, pull_request_violations
from codescene_contract.reading import triggers

REPOSITORY: typ.Final[str] = "leynos/wildside-engine"
ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[Path] = ROOT / ".github" / "workflows"
PUBLISHER: typ.Final[str] = "coverage-main.yml"
PUBLISHER_TRIGGERS: typ.Final[frozenset[str]] = frozenset({"push", "workflow_dispatch"})


@pytest.fixture(scope="module")
def documents() -> dict[str, Document]:
    """Return this repository's workflows and local actions, parsed strictly."""
    return read_workflows(WORKFLOWS) | read_actions(ROOT)


@pytest.fixture(scope="module")
def publisher(documents: dict[str, Document]) -> Document:
    """Return the one workflow that contacts CodeScene."""
    name, document = find_publisher(documents)
    assert name == PUBLISHER, name
    return document


def test_nothing_a_pull_request_starts_reaches_codescene(
    documents: dict[str, Document],
) -> None:
    """The pull-request closure names no host, credential, client or uploader."""
    found = pull_request_violations(documents, REPOSITORY)
    assert not found, found


def test_the_publisher_runs_only_on_a_push_to_main(publisher: Document) -> None:
    """The publisher answers a push to main and an optional dispatch only."""
    found = trigger_violations(publisher)
    assert not found, found


def test_the_publisher_answers_exactly_its_triggers(publisher: Document) -> None:
    """Losing the dispatch or gaining a trigger is a change to review."""
    found = triggers(publisher)
    assert found == PUBLISHER_TRIGGERS, found


def test_the_publisher_never_cancels(publisher: Document) -> None:
    """One ref-keyed group: runs never overlap and none is cancelled."""
    found = concurrency_violations(publisher)
    assert not found, found


def test_the_token_check_runs_its_one_command(publisher: Document) -> None:
    """The check step records whether the secret is set, binding nothing."""
    found = check_step_violations(publisher)
    assert not found, found


def test_the_upload_is_guarded_and_bound(publisher: Document) -> None:
    """The upload step carries both guards and passes the token directly."""
    found = upload_step_violations(publisher)
    assert not found, found


def test_the_token_reaches_only_its_two_uses(publisher: Document) -> None:
    """No `env` at any level, and no other step, holds the credential."""
    found = token_scope_violations(publisher)
    assert not found, found


def test_the_publisher_grants_no_workflow_scope(publisher: Document) -> None:
    """The workflow-level token holds no scope; the upload job opts in."""
    found = permissions_violations(publisher)
    assert not found, found


def test_the_upload_reads_what_the_publisher_writes(publisher: Document) -> None:
    """The upload's path and format are the coverage step's output."""
    found = wiring_violations(publisher)
    assert not found, found


def test_nothing_can_skip_the_publisher_on_a_push(publisher: Document) -> None:
    """No job and no coverage step of the publisher carries a condition."""
    found = condition_violations(publisher)
    assert not found, found


def test_the_retired_checksum_is_gone(documents: dict[str, Document]) -> None:
    """No workflow names the installer checksum or refreshes it."""
    found = retired_checksum_violations(documents)
    assert not found, found


def test_pull_request_lanes_ratchet_without_publishing(
    documents: dict[str, Document],
) -> None:
    """Every pull-request coverage lane ratchets and uploads no artefact."""
    closure = pull_request_closure(documents, REPOSITORY)
    found = pull_request_lane_violations(closure)
    assert not found, found


def test_only_the_publisher_writes_the_baseline(
    documents: dict[str, Document],
) -> None:
    """Coverage elsewhere is guarded to pull requests, so main has one writer."""
    found = second_writer_violations(documents, PUBLISHER, REPOSITORY)
    assert not found, found


def test_the_publisher_measures_what_each_lane_measures(
    documents: dict[str, Document], publisher: Document
) -> None:
    """The baseline is taken over the selection and pin the lanes use."""
    closure = pull_request_closure(documents, REPOSITORY)
    found = publisher_lane_violations(publisher, closure)
    assert not found, found

"""Contract tests that the default test selection runs once per event.

`make test` runs nextest over ``--workspace --all-targets --features
test-support``. The coverage run, in ``ci.yml`` on a pull request and in
``coverage-main.yml`` on a push to main, runs nextest over the same workspace
and features with the action's default targets, which leave out only the
bench targets. Measured at a37e992, the two selections are 388 and 385 tests,
and the three extra cases are ``wildside-solver-vrp``'s Criterion benches.

The ``build`` job used to run `make test` on both events as well, so 385 tests
ran twice on every pull request and every push. It now runs `make
test-benches`, the bench targets alone. These tests hold that split:

- the ``build`` job runs no whole-suite command and runs the bench step
  unconditionally;
- `test-benches` selects only bench binaries, under the same features as
  `test`;
- both coverage steps keep the features `test` uses and leave the bench
  targets out, so the bench step is not itself a duplicate;
- every feature-matrix leg names a feature set of its own, since a leg with
  no flags would repeat the default selection a third time.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shlex
from pathlib import Path
from typing import Any

import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = REPOSITORY_ROOT / ".github" / "workflows"
MAKEFILE = REPOSITORY_ROOT / "Makefile"

#: A command that runs the whole default selection, or could.
WHOLE_SUITE = re.compile(
    r"\bmake\s+(\S+=\S+\s+)*test(?![-\w])|\bnextest\s+run\b|\bcargo\s+test\b"
)
BENCH_STEP_COMMAND = "make test-benches"
TEST_BENCHES_RECIPE = (
    'RUSTFLAGS="-D warnings" $(CARGO) nextest run --workspace --benches '
    "--features test-support -E 'kind(bench)' $(BUILD_JOBS)"
)
COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"


def _workflow(name: str) -> dict[str, Any]:
    """Parse one workflow document."""
    parsed = yaml.safe_load((WORKFLOWS / name).read_text(encoding="utf-8"))
    assert isinstance(parsed, dict), f"{name} must parse to a mapping"
    return parsed


def _recipe(target: str) -> list[str]:
    """Return one Makefile target's recipe lines, without their tab."""
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    starts = [i for i, line in enumerate(lines) if line.startswith(f"{target}:")]
    assert len(starts) == 1, f"the Makefile must define {target} exactly once"
    recipe = []
    for line in lines[starts[0] + 1 :]:
        if not line.startswith("\t"):
            break
        recipe.append(line[1:])
    return recipe


def _features(command: str) -> list[str]:
    """Return the values of every ``--features`` option in a command."""
    words = shlex.split(command)
    return [words[i + 1] for i, word in enumerate(words[:-1]) if word == "--features"]


def _coverage_steps() -> list[tuple[str, dict[str, Any]]]:
    """Return every coverage generation step, labelled by workflow and job."""
    found = []
    for name in ("ci.yml", "coverage-main.yml"):
        for job_name, job in _workflow(name)["jobs"].items():
            found.extend(
                (f"{name}/{job_name}", step)
                for step in job.get("steps") or []
                if str(step.get("uses", "")).startswith(COVERAGE_ACTION)
            )
    return found


def test_the_build_job_runs_only_the_bench_targets() -> None:
    """Refuse a whole-suite command in ``build``; require the bench step."""
    steps = _workflow("ci.yml")["jobs"]["build"]["steps"]
    commands = [str(step.get("run", "")).strip() for step in steps]

    whole_suite = [
        command
        for command in commands
        if WHOLE_SUITE.search(command) and command != BENCH_STEP_COMMAND
    ]
    assert not whole_suite, f"build repeats the coverage run with {whole_suite!r}"

    bench_steps = [step for step in steps if step.get("run") == BENCH_STEP_COMMAND]
    assert len(bench_steps) == 1, "build must run `make test-benches` once"
    assert "if" not in bench_steps[0], "the bench step must run on every event"


def test_test_benches_selects_only_bench_binaries_under_the_test_features() -> None:
    """Hold the recipe, and tie its features to `make test`'s."""
    assert _recipe("test-benches") == [TEST_BENCHES_RECIPE]
    (test_command,) = _recipe("test")
    assert _features(test_command) == _features(TEST_BENCHES_RECIPE), (
        "test-benches must build the benches with the features test uses"
    )


def test_coverage_runs_the_test_features_without_the_bench_targets() -> None:
    """Keep both coverage runs on the selection the bench step completes."""
    (test_command,) = _recipe("test")
    steps = _coverage_steps()
    assert len(steps) == 2, f"expected two coverage steps, found {len(steps)}"
    for where, step in steps:
        inputs = step.get("with") or {}
        assert [str(inputs.get("features", ""))] == _features(test_command), (
            f"{where} must measure the features `make test` uses"
        )
        assert str(inputs.get("use-cargo-nextest")) == "true", (
            f"{where} must run nextest"
        )
        assert str(inputs.get("all-targets", "false")) == "false", (
            f"{where} would run the bench targets the build job runs"
        )


@pytest.mark.parametrize(
    "leg",
    _workflow("ci.yml")["jobs"]["feature-matrix"]["strategy"]["matrix"]["include"],
    ids=lambda leg: str(leg.get("name")),
)
def test_every_feature_matrix_leg_names_its_own_feature_set(
    leg: dict[str, Any],
) -> None:
    """Refuse a leg with no flags, which would rerun the default selection."""
    assert str(leg.get("test_flags", "")).strip(), (
        f"feature-matrix leg {leg.get('name')!r} repeats the default selection"
    )

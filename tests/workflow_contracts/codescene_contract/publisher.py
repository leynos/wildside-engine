"""Rules for the one push-to-main workflow allowed to upload to CodeScene.

The publisher is found rather than named, as the only workflow in the tree
that contacts CodeScene at all, so a second uploader cannot hide behind
the first. Each rule returns its findings as text; an empty list is
compliance.

The credential reaches exactly two places. A check step, which binds
nothing and runs one exact command, records whether the secret is set;
the upload step's guard reads that output and the step passes the secret
to the uploader's `access-token` input. The uploader is a composite action
that hands its step's `env` to the nested artefact and cache steps it
runs, so the token must not sit in that `env`, nor in any other.
"""

from __future__ import annotations

import re
import typing as typ

from .expressions import ConditionError, missing_terms
from .loading import Document, WorkflowReadingError
from .reach import codescene_contacts, unnamed_secret_references
from .reading import jobs, steps, texts, trigger_filters, triggers

UPLOAD_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)
PINNED_COMMIT: typ.Final[re.Pattern[str]] = re.compile(r"^[0-9a-f]{40}$")
TOKEN_INPUT: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"
CHECK_STEP_ID: typ.Final[str] = "codescene-token"

#: The check step's sole command. The expression evaluates to `true` or
#: `false` before the shell starts, so the step binds nothing and holds no
#: shell conditional that a prefix such as `false &&` could neutralize.
CHECK_COMMAND: typ.Final[str] = (
    "echo \"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> \"$GITHUB_OUTPUT\""
)

#: The only keys the check step may carry: no `if:`, `env`, `shell` or
#: `continue-on-error` can change what its one command does.
CHECK_STEP_KEYS: typ.Final[frozenset[str]] = frozenset({"name", "id", "run"})
MAIN_REF_GUARD: typ.Final[str] = "github.ref == 'refs/heads/main'"
AVAILABLE_GUARD: typ.Final[str] = f"steps.{CHECK_STEP_ID}.outputs.available == 'true'"
UPLOAD_GUARD: typ.Final[frozenset[str]] = frozenset({MAIN_REF_GUARD, AVAILABLE_GUARD})

#: The publisher's one concurrency declaration, held exactly. Keyed on the
#: ref alone, so every run for main shares one group: runs never overlap,
#: and the survivor of any replacement is the newest trigger, so uploads
#: land in commit order. Adding the event name would let an earlier
#: dispatch finish after a newer push and upload older coverage last.
PUBLISHER_CONCURRENCY: typ.Final[dict[str, object]] = {
    "group": "coverage-main-${{ github.ref }}",
    "cancel-in-progress": False,
}
PERMITTED_TRIGGERS: typ.Final[frozenset[str]] = frozenset({"push", "workflow_dispatch"})


def find_publisher(documents: dict[str, Document]) -> tuple[str, Document]:
    """Return the single workflow that contacts CodeScene.

    Raises
    ------
    WorkflowReadingError
        If none does, or more than one does.

    """
    found = [name for name, doc in documents.items() if codescene_contacts(doc)]
    if len(found) != 1:
        message = f"exactly one workflow may contact CodeScene; found {found}"
        raise WorkflowReadingError(message)
    return found[0], documents[found[0]]


def action_steps(document: Document, action: str) -> list[dict[str, object]]:
    """Return every step in a document invoking one action at any ref."""
    return [
        step
        for job in jobs(document).values()
        for step in steps(job)
        if _action_of(step) == action.casefold()
    ]


def _action_of(step: dict[str, object]) -> str:
    """Return a step's action reference without its ref, case-folded.

    GitHub resolves the owner and repository without regard to case, so a
    differently cased reference runs the same action.
    """
    return str(step.get("uses", "")).split("@", 1)[0].casefold()


def pin_of(step: dict[str, object]) -> str:
    """Return the ref after the `@` in a step's `uses:`."""
    return str(step.get("uses", "")).partition("@")[2]


def upload_step(document: Document) -> dict[str, object]:
    """Return the publisher's one upload step.

    Raises
    ------
    WorkflowReadingError
        If the uploader is invoked other than exactly once.

    """
    found = action_steps(document, UPLOAD_ACTION)
    if len(found) != 1:
        message = f"the publisher must upload exactly once; found {len(found)}"
        raise WorkflowReadingError(message)
    return found[0]


def upload_job(document: Document) -> dict[str, object]:
    """Return the job holding the publisher's one upload step."""
    step = upload_step(document)
    return next(
        job
        for job in jobs(document).values()
        if any(candidate is step for candidate in steps(job))
    )


def trigger_violations(document: Document) -> list[str]:
    """Refuse any trigger but a push to main and an optional dispatch."""
    found = [
        f"trigger {name!r} is not permitted"
        for name in sorted(triggers(document) - PERMITTED_TRIGGERS)
    ]
    if "push" not in triggers(document):
        found.append("the publisher does not run on a push")
    if trigger_filters(document, "push") not in (
        {"branches": ["main"]},
        {"branches": "main"},
    ):
        found.append("the push trigger must filter on exactly `branches: [main]`")
    return found


def concurrency_violations(document: Document) -> list[str]:
    """Require the one ref-keyed, never-cancelling group, held exactly.

    A cancelled run abandons both its upload and its baseline write, and a
    job-level group beside the workflow's would be a second key through
    which runs could overlap, so the workflow declares the group and no
    job declares another.
    """
    declared = document.get("concurrency")
    found = (
        []
        if declared == PUBLISHER_CONCURRENCY
        else [f"concurrency is {declared!r}, not {PUBLISHER_CONCURRENCY!r}"]
    )
    return found + [
        f"job {name} declares its own concurrency"
        for name, job in jobs(document).items()
        if "concurrency" in job
    ]


def check_step(document: Document) -> dict[str, object] | None:
    """Return the upload job's one token check step, or None if it has not one."""
    found = [
        step for step in steps(upload_job(document)) if step.get("id") == CHECK_STEP_ID
    ]
    return found[0] if len(found) == 1 else None


def check_step_violations(document: Document) -> list[str]:
    """Require the token check to run its one exact command before the upload.

    Without the check step the upload guard reads a missing output as
    empty and skips for ever, with nothing failing anywhere, so the step
    is asserted positively rather than inferred from the guard.
    """
    job_steps = steps(upload_job(document))
    check = check_step(document)
    if check is None:
        return [f"the upload job must hold one `{CHECK_STEP_ID}` step"]
    extra = sorted(str(key) for key in check if key not in CHECK_STEP_KEYS)
    problems = [f"the check step may not declare {key!r}" for key in extra]
    problems.extend(_run_defaults_violations(document))
    if str(check.get("run", "")).strip() != CHECK_COMMAND:
        problems.append(f"the check step must run exactly {CHECK_COMMAND!r}")
    if _position(job_steps, check) > _position(job_steps, upload_step(document)):
        problems.append("the check step must run before the upload step")
    return problems


def _run_defaults_violations(document: Document) -> list[str]:
    """Refuse `defaults.run` on the workflow or the upload job.

    A default shell or working directory reshapes the check step as a
    step-level `shell` would: `bash -c 'exit 0; {0}'` skips its command, the
    output is never written, and the upload skips for ever.
    """
    holders = (("workflow", document), ("upload job", upload_job(document)))
    return [
        f"the {scope} may not set `defaults.run`; it reshapes the check step"
        for scope, holder in holders
        if _sets_run_defaults(holder)
    ]


def _sets_run_defaults(holder: dict[object, object]) -> bool:
    """Return whether a workflow or job declares `defaults.run`."""
    defaults = holder.get("defaults")
    return isinstance(defaults, dict) and "run" in defaults


def _position(job_steps: list[dict[str, object]], step: dict[str, object]) -> int:
    """Return a step's index by identity, since two steps may compare equal."""
    return next(index for index, other in enumerate(job_steps) if other is step)


def _guard_violations(step: dict[str, object]) -> list[str]:
    """Require the ref and availability guard as whole `&&` terms."""
    try:
        missing = missing_terms(step.get("if"), UPLOAD_GUARD)
    except ConditionError as error:
        return [str(error)]
    return [f"the upload guard lacks {term!r}" for term in missing]


def upload_step_violations(document: Document) -> list[str]:
    """Require the upload step's mode, pin, guard and direct token input."""
    step = upload_step(document)
    inputs = step.get("with") or {}
    if not isinstance(inputs, dict):
        return ["the upload step's `with` must be a mapping"]
    expected = {
        "mode": "upload",
        "access-token": TOKEN_INPUT,
    }
    found = [
        f"{name} is {inputs.get(name)!r}, not {wanted!r}"
        for name, wanted in expected.items()
        if inputs.get(name) != wanted
    ]
    if not PINNED_COMMIT.match(pin_of(step)):
        found.append(f"the uploader is not pinned to a commit: {step.get('uses')!r}")
    return found + _guard_violations(step)


def _without_permitted_references(document: Document) -> list[object]:
    """Return the publisher's parts with the two permitted references removed.

    The check step's command and the upload step's `access-token` input are
    the only places the credential may appear; everything else, the
    workflow's and each job's `env` included, is returned for the sweep.
    """
    upload = upload_step(document)
    check = check_step(document)
    parts: list[object] = [
        {key: value for key, value in document.items() if key != "jobs"}
    ]
    for job in jobs(document).values():
        parts.append({key: value for key, value in job.items() if key != "steps"})
        parts.extend(_permitted_removed(step, check, upload) for step in steps(job))
    return parts


def _permitted_removed(
    step: dict[str, object],
    check: dict[str, object] | None,
    upload: dict[str, object],
) -> dict[str, object]:
    """Return one step with the reference it is permitted to hold removed."""
    if step is check:
        return {key: value for key, value in step.items() if key != "run"}
    if step is upload:
        return _upload_without_token_input(step)
    return step


def _upload_without_token_input(step: dict[str, object]) -> dict[str, object]:
    """Return the upload step with its `access-token` input removed."""
    inputs = step.get("with")
    if not isinstance(inputs, dict):
        return step
    kept = {key: value for key, value in inputs.items() if key != "access-token"}
    return {**step, "with": kept}


def token_scope_violations(document: Document) -> list[str]:
    """Refuse the credential anywhere in the publisher but its two uses.

    An `env` binding at any level is refused with the rest: the uploader
    binds the token itself from its input, and a step-level binding would
    reach the nested steps of that composite action.
    """
    return [
        f"the credential appears outside the check and the upload input: {text!r}"
        for part in _without_permitted_references(document)
        for text in texts(part)
        if "cs_access_token" in text.casefold()
    ] + [
        f"the publisher reads secrets without naming one: {text!r}"
        for text in unnamed_secret_references(document)
    ]


def permissions_violations(document: Document) -> list[str]:
    """Require the publisher's workflow-level token to hold no scope.

    Each job then opts in to what it needs; a scope granted at workflow
    level reaches every job, the check and upload steps included.
    """
    declared = document.get("permissions")
    return (
        []
        if declared == {}
        else [f"workflow permissions are {declared!r}, not {{}}"]
    )


def wiring_violations(document: Document) -> list[str]:
    """Require the upload to read the file, in the format, the publisher writes.

    Otherwise the upload sends nothing useful, or fails, while every other
    clause passes.
    """
    generators = action_steps(document, COVERAGE_ACTION)
    written = [
        (_input(step, "output-path"), _input(step, "format")) for step in generators
    ]
    upload = upload_step(document)
    read = (_input(upload, "path"), _input(upload, "format"))
    return (
        []
        if read in written
        else [f"the upload reads {read!r}; the publisher writes {written!r}"]
    )


def _input(step: dict[str, object], name: str) -> object:
    """Return one `with` input of a step, or None when it has none."""
    inputs = step.get("with")
    return inputs.get(name) if isinstance(inputs, dict) else None


def retired_checksum_violations(documents: dict[str, Document]) -> list[str]:
    """Refuse the retired installer checksum and its refresher anywhere."""
    found = [
        f"{name} names {text!r}"
        for name, document in sorted(documents.items())
        for text in texts(document)
        if re.search(r"installer-checksum|codescene_cli_sha256", text.casefold())
    ]
    return found + [
        f"{name} is the retired checksum refresher"
        for name in documents
        if name.startswith("get-codescene-sha.")
    ]

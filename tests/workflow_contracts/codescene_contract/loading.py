"""Load GitHub Actions workflows strictly enough for a contract to trust.

This is the one filesystem boundary in the package. A workflow that does
not parse, repeats a key, or is not a mapping is refused with its file
named, and an empty directory is the reader failing rather than the
repository complying.
"""

from __future__ import annotations

import collections.abc as cabc
import re
import typing as typ

import yaml
from yaml.constructor import ConstructorError

if typ.TYPE_CHECKING:
    from pathlib import Path

#: A parsed workflow. The key type is `object` because a key need not be a
#: string: `true:` parses to the boolean `True`, and a reader handed a
#: document from a YAML 1.1 loader sees an unquoted `on:` that way too.
Document: typ.TypeAlias = dict[object, object]

#: The YAML tag PyYAML gives a resolved boolean.
_BOOL_TAG: typ.Final[str] = "tag:yaml.org,2002:bool"

#: The scalars GitHub reads as booleans. YAML 1.1 also resolves `yes`,
#: `no`, `on` and `off`, but GitHub passes those to an action as strings,
#: so an input such as `with-ratchet: yes` must stay the string it is.
_GITHUB_BOOL: typ.Final[re.Pattern[str]] = re.compile(
    r"^(?:true|True|TRUE|false|False|FALSE)$"
)

#: File suffixes GitHub runs as workflows, compared without case.
WORKFLOW_SUFFIXES: typ.Final[frozenset[str]] = frozenset({".yml", ".yaml"})


class WorkflowReadingError(Exception):
    """Raised when a workflow cannot be read into a shape the rules trust."""


class _UniqueKeyLoader(yaml.SafeLoader):
    """A `yaml.SafeLoader` refusing a mapping that declares a key twice.

    PyYAML keeps the last of two equal keys and says nothing, so a job
    declaring `runs-on` twice parses into a document holding only the
    second value while the rules read the half GitHub may not run. Only
    `true` and `false` resolve to booleans, as GitHub reads them.
    """

    yaml_implicit_resolvers: typ.ClassVar[dict[str, list[tuple[str, typ.Any]]]] = {
        first: [pair for pair in resolvers if pair[0] != _BOOL_TAG]
        for first, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
    }

    def construct_mapping(
        self, node: yaml.MappingNode, deep: bool = False
    ) -> dict[typ.Hashable, typ.Any]:
        """Construct one mapping, refusing a key already seen in it.

        Raises
        ------
        ConstructorError
            If a key appears twice or cannot be hashed, naming it and where
            it appears.

        """
        seen: set[object] = set()
        for key_node, _ in node.value:
            key = self.construct_object(key_node, deep=deep)
            if not isinstance(key, cabc.Hashable):
                context = "while constructing a mapping"
                problem = f"found unhashable key {key!r}"
                raise ConstructorError(
                    context, node.start_mark, problem, key_node.start_mark
                )
            if key in seen:
                context = "while constructing a mapping"
                problem = f"found duplicate key {key!r}"
                raise ConstructorError(
                    context, node.start_mark, problem, key_node.start_mark
                )
            seen.add(key)
        return super().construct_mapping(node, deep=deep)


_UniqueKeyLoader.add_implicit_resolver(_BOOL_TAG, _GITHUB_BOOL, list("tTfF"))


def load_workflow(text: str) -> Document:
    r"""Parse one workflow, refusing duplicate keys and non-mapping documents.

    Raises
    ------
    WorkflowReadingError
        If the text is not YAML, repeats a key, uses a key that cannot be
        hashed, or is not a mapping.

    Examples
    --------
    >>> load_workflow("on: push\njobs: {}\n")
    {'on': 'push', 'jobs': {}}

    """
    # What `yaml.load` does, spelt out so no linter mistakes the strict
    # SafeLoader subclass for an unsafe loader.
    loader = _UniqueKeyLoader(text)
    try:
        parsed = loader.get_single_data()
    except yaml.YAMLError as error:
        message = f"not a workflow document: {error}"
        raise WorkflowReadingError(message) from error
    finally:
        loader.dispose()
    if not isinstance(parsed, dict):
        message = "a workflow must parse to a top-level mapping"
        raise WorkflowReadingError(message)
    return typ.cast("Document", parsed)


def read_workflows(directory: Path) -> dict[str, Document]:
    """Return every workflow under one directory, by file name.

    Both suffixes are read, whatever their case, because GitHub runs a
    workflow named either way.

    Raises
    ------
    WorkflowReadingError
        If the directory cannot be listed or holds no workflow, or a
        workflow cannot be read or does not parse. The message names the
        directory or the file, and the I/O error, if any, is the cause.

    """
    paths = [
        path
        for path in _entries(directory)
        if path.suffix.casefold() in WORKFLOW_SUFFIXES
    ]
    if not paths:
        message = f"no workflow was read from {directory}; the reader is broken"
        raise WorkflowReadingError(message)
    return {path.name: _load_file(path) for path in paths}


def _entries(directory: Path) -> list[Path]:
    """List one directory in name order, naming it in any I/O failure."""
    try:
        return sorted(directory.iterdir())
    except OSError as error:
        message = f"{directory} could not be listed: {error}"
        raise WorkflowReadingError(message) from error


def _load_file(path: Path) -> Document:
    """Read and parse one workflow file, naming it in any failure."""
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        message = f"{path.name} could not be read: {error}"
        raise WorkflowReadingError(message) from error
    try:
        return load_workflow(text)
    except WorkflowReadingError as error:
        message = f"{path.name}: {error}"
        raise WorkflowReadingError(message) from error

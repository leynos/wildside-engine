"""Readers and rules for the CV-005 CodeScene contract over the workflows.

Main owns CodeScene: one push-to-main publisher uploads coverage and
writes the ratchet baseline, and nothing a pull request can start talks to
CodeScene or holds its credential.

The readers here are pure over supplied text or parsed documents, apart
from `loading.read_workflows`, which is the one filesystem boundary. That
split is what lets the rule tests ask what a rule makes of a workflow this
repository does not contain: the real files use one spelling of
everything, so they cannot tell a working reader from a broken one.
"""

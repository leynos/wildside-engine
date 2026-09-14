# Environment-policy scan samples

Sample sources for `tests/clippy_env_policy_source_tests.rs`. Each is loaded
with `include_str!` and handed to the scan as text; none is compiled.

The `.rs.txt` extension is deliberate. A tracked `.rs` file here would be
picked up by the workspace scan itself, reported as an offence, and would force
a second entry in the exemption list that `only_the_ui_fixture_is_exempt`
exists to keep closed.

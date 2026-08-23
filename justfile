regen-schema:
    capnp compile -orust:crates/claimdag/src --src-prefix=schema schema/claimdag.capnp

test-py:
    uv run --with '.[test]' pytest -q tests

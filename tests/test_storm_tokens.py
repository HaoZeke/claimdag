"""Shared Tokyo Night Storm tokens. Not Night #1a1b26."""

from pathlib import Path

TUI_DIR = Path(__file__).resolve().parents[1] / "claimdag_tui"


def test_shared_snippet_is_storm_not_night() -> None:
    shared = (TUI_DIR / "tokyo_night_storm.tcss").read_text(encoding="utf-8")
    assert "#24283b" in shared
    assert "(36, 40, 59)" in shared
    assert "#c0caf5" in shared
    assert "#7aa2f7" in shared
    assert "$panel:" not in shared


def test_widget_css_is_layout_not_pad() -> None:
    widgets = (TUI_DIR / "storm.tcss").read_text(encoding="utf-8")
    shared = (TUI_DIR / "tokyo_night_storm.tcss").read_text(encoding="utf-8")
    assert "height: 1fr" in widgets
    assert "layout: vertical" in shared
    assert "Tree" in widgets
    assert "#24283b" in widgets
    assert "$panel" not in shared
    assert "$panel" not in widgets



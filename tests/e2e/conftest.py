"""Pytest fixtures for E2E tests."""

import pytest

from tauri_app import TauriApp


@pytest.fixture
def app():
    """Provide a TauriApp instance that is cleaned up after the test."""
    tauri = TauriApp()
    yield tauri
    tauri.stop()

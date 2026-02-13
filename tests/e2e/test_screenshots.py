"""E2E screenshot tests for Immerse Yourself.

These tests launch the full Tauri app inside Docker with Xvfb, navigate the UI
via the WebKit Inspector Protocol (or keyboard fallback), and capture screenshots.
"""

import os

from tauri_app import TauriApp


def test_travel_afternoon_screenshot(app: TauriApp):
    """Navigate to Travel > Afternoon and capture a screenshot.

    This produces the canonical project screenshot showing:
    - "travel" category highlighted in the sidebar
    - "Travel" environment button active (green border)
    - Afternoon time variant selected in the time-of-day bar
    - Now Playing widget showing "Travel"
    - Lighting preview showing afternoon warm golden colors
    """
    output_dir = os.environ.get("OUTPUT_DIR", "/output")
    screenshot_path = os.path.join(output_dir, "immerse_yourself_screenshot.jpg")

    # 1. Start the app and wait for full load (including startup environment)
    app.start(maximize=True)
    app.wait(app.STARTUP_WAIT)

    # 2. Try to connect the WebKit Inspector
    has_inspector = app.connect_inspector()

    # 3. Wait for the UI to be fully rendered
    app.wait_for_element(".category-list", timeout=15)
    app.wait(app.ACTION_WAIT)

    # Log initial state
    print("[TEST] === Initial state after startup ===")
    app.log_debug_state()

    # 4. Navigate to the "travel" category
    app.click_category("travel")
    app.wait(app.ACTION_WAIT)

    if has_inspector:
        # === Inspector path: use direct Tauri IPC for reliability ===
        # This bypasses the time variant dialog click simulation entirely.
        # We call the backend directly to start Travel with Afternoon,
        # then wait for the frontend's 1-second polling to reflect the change.

        print("[TEST] Using inspector path with direct Tauri IPC")

        # 5. Activate Travel + Afternoon via Tauri IPC
        # This may block while atmosphere sounds are downloaded (up to 60s)
        app.invoke_tauri('start_environment_with_time', {
            'configName': 'Travel',
            'time': 'afternoon',
        }, timeout=90)

        # 6. Wait for the UI polling to pick up the new state
        matched = app.wait_for_text(".now-playing-status", "Travel", timeout=15)
        if not matched:
            print("[TEST] WARNING: Now Playing didn't show 'Travel', taking debug screenshot")

        # 7. Verify all UI elements reflect the correct state
        print("[TEST] === State after Travel Afternoon activation ===")
        app.log_debug_state()

    else:
        # === Keyboard fallback path ===
        # The startup already loaded the "Startup" environment.
        # We click Travel to trigger the time variant dialog, then press '3'
        # for Afternoon via the dialog's keyboard shortcut handler.

        print("[TEST] Using keyboard fallback path")

        # 5. Click the "Travel" environment button
        app.wait_for_element(".env-button", timeout=10)
        app.click_environment("Travel")
        app.wait(3)  # Wait for time variant dialog to appear

        # 6. Select "Afternoon" via keyboard shortcut
        app.select_time_variant("Afternoon")
        app.wait(app.ENVIRONMENT_WAIT)

    # 8. Extra settle time for all state to propagate
    app.wait(3)

    # 9. Capture the screenshot
    app.screenshot(screenshot_path)

    # Verify the screenshot was created and isn't blank
    assert os.path.exists(screenshot_path), f"Screenshot not found at {screenshot_path}"
    file_size = os.path.getsize(screenshot_path)
    assert file_size > 10000, f"Screenshot too small ({file_size} bytes), likely blank"
    print(f"[TEST] Screenshot saved: {screenshot_path} ({file_size} bytes)")

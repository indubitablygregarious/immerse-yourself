#!/usr/bin/env python3
"""Run the screenshot test directly on the host (no Docker needed).

Requires: built binary, DISPLAY set, ImageMagick (import), Python websockets.
Does NOT require: xdotool, scrot, Xvfb.

Usage:
    python3 tests/e2e/run_local.py [--binary path/to/immerse-tauri]
"""

import asyncio
import json
import os
import re
import signal
import subprocess
import sys
import time
import urllib.request

import websockets

PROJECT_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Default binary paths to search
BINARY_PATHS = [
    os.path.join(PROJECT_DIR, "rust/target/release/immerse-tauri"),
    os.path.join(PROJECT_DIR, "rust/target/debug/immerse-tauri"),
]

INSPECTOR_HOST = "127.0.0.1"
INSPECTOR_PORT = 3030


def find_binary(override=None):
    if override and os.path.isfile(override):
        return override
    for p in BINARY_PATHS:
        if os.path.isfile(p):
            return p
    return None


# Global state for the Target-based inspector protocol
_target_id = None
_msg_id = 0
_inner_msg_id = 0


def connect_inspector(max_attempts=20):
    """Connect to the WebKit Inspector and return (ws, loop).

    Also discovers the target ID needed for Target.sendMessageToTarget.
    """
    global _target_id
    base_url = f"http://{INSPECTOR_HOST}:{INSPECTOR_PORT}"

    for attempt in range(max_attempts):
        try:
            with urllib.request.urlopen(base_url, timeout=3) as resp:
                html = resp.read().decode()
                print(f"[Inspector] HTTP responded (attempt {attempt + 1})")

                # Primary: /socket/N/N/Type pattern (WebKitGTK 2.40+)
                socket_match = re.search(r'(/socket/\d+/\d+/\w+)', html)
                if socket_match:
                    path = socket_match.group(1)
                    ws_url = f"ws://{INSPECTOR_HOST}:{INSPECTOR_PORT}{path}"
                    print(f"[Inspector] Connecting to WebSocket: {ws_url}")
                    loop = asyncio.new_event_loop()
                    ws = loop.run_until_complete(
                        websockets.connect(ws_url, max_size=10*1024*1024, ping_timeout=120)
                    )
                    print("[Inspector] WebSocket connected!")

                    # Discover target ID
                    _discover_target(ws, loop)
                    return ws, loop

                # Fallback: href links
                patterns = [
                    r'href=["\'](/Main/\d+)["\']',
                    r'href=["\'](/Page/\d+)["\']',
                    r'href=["\']([^"\']*?/\d+)["\']',
                ]
                for pattern in patterns:
                    match = re.search(pattern, html)
                    if match:
                        path = match.group(1)
                        ws_url = f"ws://{INSPECTOR_HOST}:{INSPECTOR_PORT}{path}"
                        print(f"[Inspector] Connecting to WebSocket: {ws_url}")
                        loop = asyncio.new_event_loop()
                        ws = loop.run_until_complete(
                            websockets.connect(ws_url, max_size=10*1024*1024, ping_timeout=120)
                        )
                        print("[Inspector] WebSocket connected!")
                        _discover_target(ws, loop)
                        return ws, loop
                print("[Inspector] No targets found in HTML yet")
        except Exception as e:
            if attempt == max_attempts - 1:
                print(f"[Inspector] Failed after {max_attempts} attempts: {e}")
        time.sleep(1)
    return None, None


def _discover_target(ws, loop):
    """Read initial WebSocket events to find the target ID."""
    global _target_id, _msg_id

    async def _discover():
        global _target_id, _msg_id
        deadline = time.time() + 5
        while time.time() < deadline:
            try:
                resp = await asyncio.wait_for(ws.recv(), timeout=1.0)
                data = json.loads(resp)
                if data.get("method") == "Target.targetCreated":
                    info = data["params"]["targetInfo"]
                    _target_id = info["targetId"]
                    print(f"[Inspector] Target discovered: {_target_id} "
                          f"(type: {info.get('type', 'unknown')})")
                    return
            except asyncio.TimeoutError:
                continue

        # Fallback: request targets explicitly
        _msg_id += 1
        await ws.send(json.dumps({"id": _msg_id, "method": "Target.getTargets"}))
        deadline = time.time() + 3
        while time.time() < deadline:
            try:
                resp = await asyncio.wait_for(ws.recv(), timeout=1.0)
                data = json.loads(resp)
                if data.get("method") == "Target.targetCreated":
                    info = data["params"]["targetInfo"]
                    _target_id = info["targetId"]
                    print(f"[Inspector] Target from getTargets: {_target_id}")
                    return
                if data.get("id") == _msg_id and "result" in data:
                    targets = data["result"].get("targetList", [])
                    if targets:
                        _target_id = targets[0]["targetId"]
                        print(f"[Inspector] Target from list: {_target_id}")
                        return
            except asyncio.TimeoutError:
                continue

    loop.run_until_complete(_discover())
    if not _target_id:
        print("[Inspector] WARNING: No target ID found, JS evaluation will fail")


def evaluate_js(ws, loop, expression, timeout=10.0, await_promise=False):
    """Execute JS via Target.sendMessageToTarget wrapping Runtime.evaluate."""
    global _msg_id, _inner_msg_id

    if not _target_id:
        raise RuntimeError("No target ID discovered")

    _inner_msg_id += 1
    inner_id = _inner_msg_id
    inner_msg = {
        "id": inner_id,
        "method": "Runtime.evaluate",
        "params": {
            "expression": expression,
            "returnByValue": True,
        },
    }
    if await_promise:
        inner_msg["params"]["awaitPromise"] = True

    _msg_id += 1
    outer_id = _msg_id
    outer_msg = {
        "id": outer_id,
        "method": "Target.sendMessageToTarget",
        "params": {
            "targetId": _target_id,
            "message": json.dumps(inner_msg),
        },
    }

    async def _eval():
        await ws.send(json.dumps(outer_msg))
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                response = await asyncio.wait_for(
                    ws.recv(), timeout=min(1.0, deadline - time.time())
                )
                data = json.loads(response)

                # Result comes via Target.dispatchMessageFromTarget
                if data.get("method") == "Target.dispatchMessageFromTarget":
                    inner_resp = json.loads(data["params"]["message"])
                    if inner_resp.get("id") == inner_id:
                        result_obj = inner_resp.get("result", {})
                        if "exceptionDetails" in inner_resp:
                            exc = inner_resp["exceptionDetails"]
                            text = exc.get("text", str(exc))
                            raise RuntimeError(f"JS error: {text}")
                        return result_obj.get("result", {}).get("value")
                # Skip outer ACK and other events
            except asyncio.TimeoutError:
                continue
        raise TimeoutError(f"No response for inner message {inner_id}")

    return loop.run_until_complete(_eval())


def invoke_tauri(ws, loop, command, args=None, timeout=30.0):
    """Call Tauri IPC directly.

    awaitPromise doesn't work through WebKitGTK's Target proxy, so we use a
    polling approach: fire the async call, store result in a global, then poll.
    """
    args_json = json.dumps(args) if args else '{}'

    # Fire the async call and store result in a global
    setup_js = f"""
    (() => {{
        window.__e2e_pending = true;
        window.__e2e_result = null;
        (async () => {{
            try {{
                const result = await window.__TAURI_INTERNALS__.invoke('{command}', {args_json});
                window.__e2e_result = {{ success: true, result: JSON.parse(JSON.stringify(result ?? null)) }};
            }} catch (e) {{
                window.__e2e_result = {{ success: false, error: String(e) }};
            }} finally {{
                window.__e2e_pending = false;
            }}
        }})();
        return 'started';
    }})()
    """
    evaluate_js(ws, loop, setup_js, timeout=5)
    print(f"[Tauri] invoke '{command}' started, polling for result...")

    # Poll for the result
    start = time.time()
    while time.time() - start < timeout:
        result = evaluate_js(ws, loop,
            "window.__e2e_pending ? null : window.__e2e_result", timeout=5)
        if result is not None:
            if isinstance(result, dict) and not result.get('success'):
                raise RuntimeError(f"Tauri invoke '{command}' failed: {result.get('error')}")
            print(f"[Tauri] invoke '{command}' succeeded")
            return result.get('result') if isinstance(result, dict) else result
        time.sleep(1)

    raise TimeoutError(f"Tauri invoke '{command}' timed out after {timeout}s")


def get_text(ws, loop, selector):
    """Get element text content."""
    js = f"document.querySelector('{selector}')?.textContent?.trim() ?? null"
    return evaluate_js(ws, loop, js)


def wait_for_text(ws, loop, selector, expected, timeout=15.0):
    """Wait for element text to match."""
    start = time.time()
    while time.time() - start < timeout:
        text = get_text(ws, loop, selector)
        if text and expected.lower() in text.lower():
            print(f"[OK] '{selector}' contains '{expected}' (got: '{text}')")
            return True
        time.sleep(0.5)
    actual = get_text(ws, loop, selector)
    print(f"[WARN] '{selector}' expected '{expected}', got '{actual}' after {timeout}s")
    return False


def wait_for_element(ws, loop, selector, timeout=15.0):
    """Wait for element to exist."""
    start = time.time()
    while time.time() - start < timeout:
        result = evaluate_js(ws, loop, f"!!document.querySelector('{selector}')")
        if result:
            return True
        time.sleep(0.5)
    raise TimeoutError(f"Element '{selector}' not found within {timeout}s")


def log_state(ws, loop):
    """Print current UI state."""
    now_playing = get_text(ws, loop, ".now-playing-status")
    time_selected = get_text(ws, loop, ".time-button.selected .time-label")
    header = get_text(ws, loop, ".content-header h1")
    has_active_btn = evaluate_js(ws, loop, "!!document.querySelector('.env-button.active')")
    has_active_light = evaluate_js(ws, loop, "!!document.querySelector('.lighting-preview-widget.active')")
    print(f"  Category header: {header}")
    print(f"  Now playing: {now_playing}")
    print(f"  Selected time: {time_selected}")
    print(f"  Active env button: {has_active_btn}")
    print(f"  Active lighting widget: {has_active_light}")


def find_window_id(name="Immerse Yourself"):
    """Find X11 window ID by name using xwininfo."""
    result = subprocess.run(
        ["xwininfo", "-root", "-tree"],
        capture_output=True, text=True, timeout=5,
        env={**os.environ, "DISPLAY": os.environ.get("DISPLAY", ":1")},
    )
    if result.returncode != 0:
        return None
    for line in result.stdout.splitlines():
        if name in line and "child" not in line.lower():
            # Extract hex window ID from lines like: 0x4200003 "Immerse Yourself": ...
            match = re.search(r'(0x[0-9a-fA-F]+)\s+"', line.strip())
            if match:
                return match.group(1)
    return None


def take_screenshot(path):
    """Capture screenshot of just the app window using ImageMagick import."""
    wid = find_window_id()
    if wid:
        print(f"[Screenshot] Capturing window {wid}")
        result = subprocess.run(
            ["import", "-window", wid, path],
            capture_output=True, text=True, timeout=10,
        )
    else:
        print("[Screenshot] Window not found, capturing full screen")
        result = subprocess.run(
            ["import", "-window", "root", path],
            capture_output=True, text=True, timeout=10,
        )
    if result.returncode == 0:
        size = os.path.getsize(path)
        print(f"[Screenshot] Saved: {path} ({size} bytes)")
    else:
        print(f"[Screenshot] Failed: {result.stderr}")


def main():
    binary_override = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1].startswith("/") else None
    binary = find_binary(binary_override)
    if not binary:
        print("ERROR: No binary found. Run 'make build' first.")
        sys.exit(1)

    print(f"[App] Binary: {binary}")
    print(f"[App] DISPLAY: {os.environ.get('DISPLAY', 'not set')}")

    # Start the app - only use WEBKIT_INSPECTOR_HTTP_SERVER
    # (WEBKIT_INSPECTOR_SERVER would try to bind the same port and fail)
    env = os.environ.copy()
    env["WEBKIT_INSPECTOR_HTTP_SERVER"] = f"{INSPECTOR_HOST}:{INSPECTOR_PORT}"
    env.pop("WEBKIT_INSPECTOR_SERVER", None)
    env["RUST_LOG"] = "info"
    env["GTK_A11Y"] = "none"

    proc = subprocess.Popen(
        [binary], env=env, cwd=PROJECT_DIR,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    print(f"[App] Started PID {proc.pid}")

    try:
        # Wait for the app to load
        print("[App] Waiting 12s for startup...")
        time.sleep(12)

        # Connect inspector
        ws, loop = connect_inspector()
        if not ws:
            print("ERROR: Could not connect to WebKit Inspector")
            sys.exit(1)

        # Wait for UI
        wait_for_element(ws, loop, ".category-list", timeout=15)
        print("[OK] UI rendered")
        time.sleep(2)

        # Log initial state
        print("=== Initial state ===")
        log_state(ws, loop)

        # Click travel category
        js = """
        (() => {
            const item = document.querySelector('[data-category="travel"] .category-item');
            if (item) { item.click(); return true; }
            return false;
        })()
        """
        result = evaluate_js(ws, loop, js)
        print(f"[Click] Category 'travel': {result}")
        time.sleep(2)

        # Invoke Tauri IPC to start Travel with Afternoon
        print("[Tauri] Starting Travel with Afternoon...")
        invoke_tauri(ws, loop, 'start_environment_with_time', {
            'configName': 'Travel',
            'time': 'afternoon',
        }, timeout=90)

        # Wait for UI to reflect
        wait_for_text(ws, loop, ".now-playing-status", "Travel", timeout=15)
        time.sleep(3)

        # Log final state
        print("=== Final state ===")
        log_state(ws, loop)

        # Take screenshot
        output_path = os.path.join(PROJECT_DIR, "tests/e2e/output")
        os.makedirs(output_path, exist_ok=True)
        screenshot_path = os.path.join(output_path, "immerse_yourself_screenshot.jpg")
        take_screenshot(screenshot_path)

    finally:
        proc.send_signal(signal.SIGTERM)
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
        subprocess.run(["pkill", "-9", "-f", "ffplay"], capture_output=True)
        print("[App] Terminated")


if __name__ == "__main__":
    main()

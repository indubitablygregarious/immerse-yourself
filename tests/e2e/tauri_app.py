"""Helper class for controlling the Tauri app via WebKit Inspector Protocol.

Uses WEBKIT_INSPECTOR_HTTP_SERVER (not WEBKIT_INSPECTOR_SERVER) which exposes
an HTTP + WebSocket interface compatible with the WebKit Inspector Protocol.
Falls back to xdotool keyboard shortcuts if the inspector is unavailable.
"""

import asyncio
import json
import os
import re
import signal
import subprocess
import time
import urllib.request

import websockets


class TauriApp:
    """Controls the Tauri/WebKitGTK app via WebKit Inspector or keyboard shortcuts.

    Primary: WEBKIT_INSPECTOR_HTTP_SERVER exposes an HTTP page at root with
    links to debuggable targets, and WebSocket endpoints for the inspector protocol.

    Fallback: xdotool keyboard shortcuts (Ctrl+PgDn for categories, Q-L for
    environment buttons, 1-4 for time variants).
    """

    BINARY = "/app/immerse-tauri"
    INSPECTOR_HOST = "127.0.0.1"
    INSPECTOR_PORT = 3030

    # Timing constants (seconds)
    STARTUP_WAIT = 10       # Wait for app to fully load and render
    ACTION_WAIT = 2         # Wait between UI actions
    ENVIRONMENT_WAIT = 6    # Wait after environment activation

    def __init__(self):
        self.process = None
        self._ws = None
        self._ws_url = None
        self._msg_id = 0
        self._inner_msg_id = 0
        self._loop = None
        self._window_id = None
        self._target_id = None  # WebKit Inspector target ID (e.g. "page-7")
        self._use_keyboard = False  # Set to True if inspector fails

    def start(self, maximize=True):
        """Launch the Tauri app with WebKit Inspector HTTP server enabled."""
        env = os.environ.copy()
        # Use ONLY WEBKIT_INSPECTOR_HTTP_SERVER - it serves both the HTTP listing
        # page AND WebSocket endpoints. Setting WEBKIT_INSPECTOR_SERVER too would
        # try to bind the same port twice, causing one to fail.
        env["WEBKIT_INSPECTOR_HTTP_SERVER"] = f"{self.INSPECTOR_HOST}:{self.INSPECTOR_PORT}"
        env.pop("WEBKIT_INSPECTOR_SERVER", None)
        env["RUST_LOG"] = "info"
        env["GTK_A11Y"] = "none"

        self.process = subprocess.Popen(
            [self.BINARY],
            env=env,
            cwd="/app",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        print(f"[TauriApp] Started PID {self.process.pid}")

        if maximize:
            self._wait_and_maximize()

    def _wait_and_maximize(self):
        """Wait for the window to appear and maximize it."""
        for attempt in range(30):
            result = subprocess.run(
                ["xdotool", "search", "--name", "Immerse Yourself"],
                capture_output=True, text=True,
            )
            if result.returncode == 0 and result.stdout.strip():
                self._window_id = result.stdout.strip().split("\n")[0]
                print(f"[TauriApp] Window found: {self._window_id}")
                time.sleep(1)
                # Resize and position the window
                subprocess.run(
                    ["xdotool", "windowsize", self._window_id, "1920", "1080"],
                    check=False,
                )
                subprocess.run(
                    ["xdotool", "windowmove", self._window_id, "0", "0"],
                    check=False,
                )
                # Focus the window
                subprocess.run(
                    ["xdotool", "windowfocus", "--sync", self._window_id],
                    check=False,
                )
                print("[TauriApp] Window maximized to 1920x1080")
                return
            time.sleep(1)
        raise TimeoutError("Window did not appear within 30 seconds")

    def stop(self):
        """Terminate the app process."""
        if self._ws:
            try:
                if self._loop:
                    self._loop.run_until_complete(self._ws.close())
            except Exception:
                pass
            self._ws = None

        if self.process:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            print("[TauriApp] Process terminated")
            self.process = None

    # =========================================================================
    # WebKit Inspector HTTP Protocol
    # =========================================================================

    def connect_inspector(self) -> bool:
        """Connect to the WebKit Inspector HTTP server.

        Returns True if connected, False if unavailable (will use keyboard fallback).
        """
        base_url = f"http://{self.INSPECTOR_HOST}:{self.INSPECTOR_PORT}"

        for attempt in range(15):
            try:
                with urllib.request.urlopen(base_url, timeout=3) as resp:
                    html = resp.read().decode()
                    print(f"[TauriApp] Inspector HTTP responded (attempt {attempt + 1})")

                    # Parse the HTML page to find WebSocket target URLs
                    # WebKitGTK serves an HTML page with links like:
                    #   /Main/{targetID}  or  /socket/{connID}/{targetID}/{type}
                    ws_path = self._parse_inspector_html(html)
                    if ws_path:
                        self._ws_url = f"ws://{self.INSPECTOR_HOST}:{self.INSPECTOR_PORT}{ws_path}"
                        print(f"[TauriApp] Inspector WebSocket: {self._ws_url}")
                        self._connect_ws()
                        return True
                    else:
                        print("[TauriApp] Inspector page found but no targets yet")
            except Exception as e:
                if attempt < 14:
                    pass  # Retry silently
                else:
                    print(f"[TauriApp] Inspector connection failed: {e}")
            time.sleep(1)

        print("[TauriApp] Inspector unavailable, using keyboard fallback")
        self._use_keyboard = True
        return False

    def _parse_inspector_html(self, html: str) -> str | None:
        """Parse the inspector HTML page to extract a WebSocket target path.

        WebKitGTK's inspector page contains an onclick handler with the pattern:
          window.open('Main.html?ws=' + window.location.host + '/socket/1/1/WebPage', ...)
        The WebSocket path is '/socket/{connId}/{targetId}/{type}'.
        """
        # Primary: look for /socket/N/N/Type pattern (WebKitGTK 2.40+)
        socket_match = re.search(r'(/socket/\d+/\d+/\w+)', html)
        if socket_match:
            path = socket_match.group(1)
            print(f"[TauriApp] Found inspector socket path: {path}")
            return path

        # Fallback: look for href links
        patterns = [
            r'href=["\'](/Main/\d+)["\']',
            r'href=["\'](/Page/\d+)["\']',
            r'href=["\'](/inspector/\d+)["\']',
            r'href=["\']([^"\']*?/\d+)["\']',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                path = match.group(1)
                print(f"[TauriApp] Found inspector target path: {path}")
                return path

        # Last resort: look for WebSocket URLs directly
        ws_match = re.search(r'ws://[^"\'>\s]+', html)
        if ws_match:
            return ws_match.group(0)

        # Log HTML for debugging
        print(f"[TauriApp] Inspector HTML (first 500 chars): {html[:500]}")
        return None

    def _connect_ws(self):
        """Establish the WebSocket connection and discover the target ID.

        The WebKit Inspector WebSocket connects to a Target multiplexer, NOT
        directly to the page. We must discover the target ID from the
        Target.targetCreated event, then use Target.sendMessageToTarget to
        send commands to the actual page.
        """
        self._loop = asyncio.new_event_loop()
        self._ws = self._loop.run_until_complete(
            websockets.connect(self._ws_url, max_size=10 * 1024 * 1024, ping_timeout=120)
        )
        print("[TauriApp] WebSocket connected")

        # Collect initial events to find the target ID
        async def _discover_target():
            deadline = time.time() + 5
            while time.time() < deadline:
                try:
                    resp = await asyncio.wait_for(self._ws.recv(), timeout=1.0)
                    data = json.loads(resp)
                    if data.get("method") == "Target.targetCreated":
                        target_info = data["params"]["targetInfo"]
                        self._target_id = target_info["targetId"]
                        print(f"[TauriApp] Discovered target: {self._target_id} "
                              f"(type: {target_info.get('type', 'unknown')})")
                        return
                except asyncio.TimeoutError:
                    continue

            # Fallback: explicitly request targets
            self._msg_id += 1
            await self._ws.send(json.dumps({
                "id": self._msg_id, "method": "Target.getTargets"
            }))
            deadline = time.time() + 3
            while time.time() < deadline:
                try:
                    resp = await asyncio.wait_for(self._ws.recv(), timeout=1.0)
                    data = json.loads(resp)
                    if data.get("method") == "Target.targetCreated":
                        target_info = data["params"]["targetInfo"]
                        self._target_id = target_info["targetId"]
                        print(f"[TauriApp] Discovered target via getTargets: {self._target_id}")
                        return
                    if data.get("id") == self._msg_id and "result" in data:
                        targets = data["result"].get("targetList", [])
                        if targets:
                            self._target_id = targets[0]["targetId"]
                            print(f"[TauriApp] Got target from list: {self._target_id}")
                            return
                except asyncio.TimeoutError:
                    continue

        self._loop.run_until_complete(_discover_target())
        if not self._target_id:
            print("[TauriApp] WARNING: No target ID discovered, JS evaluation will fail")

    def evaluate_js(self, expression: str, timeout: float = 10.0, await_promise: bool = False):
        """Execute JavaScript in the WebView via Target.sendMessageToTarget.

        The WebKit Inspector WebSocket connects to a Target multiplexer.
        We must wrap Runtime.evaluate inside Target.sendMessageToTarget, and
        read the result from Target.dispatchMessageFromTarget events.

        Returns the result value, or raises on error.
        Set await_promise=True for async expressions that return Promises.
        """
        if not self._ws:
            raise RuntimeError("WebSocket not connected. Call connect_inspector() first.")
        if not self._target_id:
            raise RuntimeError("No target ID. Inspector target discovery failed.")

        # Build inner message (sent to the page's Runtime domain)
        self._inner_msg_id += 1
        inner_id = self._inner_msg_id
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

        # Wrap in outer Target.sendMessageToTarget
        self._msg_id += 1
        outer_id = self._msg_id
        outer_msg = {
            "id": outer_id,
            "method": "Target.sendMessageToTarget",
            "params": {
                "targetId": self._target_id,
                "message": json.dumps(inner_msg),
            },
        }

        async def _eval():
            await self._ws.send(json.dumps(outer_msg))
            deadline = time.time() + timeout
            while time.time() < deadline:
                try:
                    response = await asyncio.wait_for(
                        self._ws.recv(), timeout=min(1.0, deadline - time.time())
                    )
                    data = json.loads(response)

                    # The actual result comes via Target.dispatchMessageFromTarget
                    if data.get("method") == "Target.dispatchMessageFromTarget":
                        inner_resp = json.loads(data["params"]["message"])
                        if inner_resp.get("id") == inner_id:
                            result = inner_resp.get("result", {})
                            if "exceptionDetails" in inner_resp:
                                exc = inner_resp["exceptionDetails"]
                                text = exc.get("text", str(exc))
                                raise RuntimeError(f"JS error: {text}")
                            return result.get("result", {}).get("value")

                    # Skip the outer ACK (just {"result": {}, "id": N})
                    # and any other events
                except asyncio.TimeoutError:
                    continue
            raise TimeoutError(f"No response for inner message {inner_id}")

        return self._loop.run_until_complete(_eval())

    # =========================================================================
    # High-level UI actions (inspector or keyboard)
    # =========================================================================

    def click_category(self, name: str):
        """Click a category in the sidebar."""
        if self._use_keyboard:
            self._click_category_keyboard(name)
        else:
            self._click_category_inspector(name)

    def click_environment(self, name: str):
        """Click an environment button by its display name."""
        if self._use_keyboard:
            self._click_environment_keyboard(name)
        else:
            self._click_environment_inspector(name)

    def select_time_variant(self, label: str):
        """Select a time variant from the dialog."""
        if self._use_keyboard:
            self._select_time_variant_keyboard(label)
        else:
            self._select_time_variant_inspector(label)

    def wait_for_element(self, selector: str, timeout: float = 10.0):
        """Poll until an element matching the CSS selector exists."""
        if self._use_keyboard:
            # Can't check DOM via keyboard, just wait
            time.sleep(2)
            return True
        start = time.time()
        while time.time() - start < timeout:
            result = self.evaluate_js(f"!!document.querySelector('{selector}')")
            if result:
                return True
            time.sleep(0.5)
        raise TimeoutError(f"Element '{selector}' not found within {timeout}s")

    def wait_for_no_element(self, selector: str, timeout: float = 10.0):
        """Poll until an element is gone from the DOM."""
        if self._use_keyboard:
            time.sleep(2)
            return True
        start = time.time()
        while time.time() - start < timeout:
            result = self.evaluate_js(f"!!document.querySelector('{selector}')")
            if not result:
                return True
            time.sleep(0.5)
        raise TimeoutError(f"Element '{selector}' still present after {timeout}s")

    def invoke_tauri(self, command: str, args: dict | None = None, timeout: float = 30.0):
        """Call a Tauri IPC command directly via __TAURI_INTERNALS__.invoke.

        This bypasses UI click simulation and directly calls the backend,
        which is more reliable for setting application state.

        awaitPromise doesn't work through WebKitGTK's Target proxy, so we use
        a polling approach: fire the async call, store result in a global, poll.
        """
        if self._use_keyboard:
            raise RuntimeError("invoke_tauri requires WebSocket inspector connection")

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
        self.evaluate_js(setup_js, timeout=5)
        print(f"[TauriApp] Tauri invoke '{command}' started, polling...")

        # Poll for the result
        start = time.time()
        while time.time() - start < timeout:
            result = self.evaluate_js(
                "window.__e2e_pending ? null : window.__e2e_result", timeout=5)
            if result is not None:
                if isinstance(result, dict) and not result.get('success'):
                    raise RuntimeError(f"Tauri invoke '{command}' failed: {result.get('error')}")
                print(f"[TauriApp] Tauri invoke '{command}' succeeded")
                return result.get('result') if isinstance(result, dict) else result
            time.sleep(1)

        raise TimeoutError(f"Tauri invoke '{command}' timed out after {timeout}s")

    def get_element_text(self, selector: str) -> str | None:
        """Get text content of an element matching the CSS selector."""
        if self._use_keyboard:
            return None
        js = f"document.querySelector('{selector}')?.textContent?.trim() ?? null"
        return self.evaluate_js(js)

    def wait_for_text(self, selector: str, expected: str, timeout: float = 15.0) -> bool:
        """Poll until element's text content contains the expected string."""
        if self._use_keyboard:
            time.sleep(3)
            return True

        start = time.time()
        while time.time() - start < timeout:
            text = self.get_element_text(selector)
            if text and expected.lower() in text.lower():
                print(f"[TauriApp] Text match: '{selector}' contains '{expected}' (got: '{text}')")
                return True
            time.sleep(0.5)

        actual = self.get_element_text(selector)
        print(f"[TauriApp] WARNING: Text not matched after {timeout}s: "
              f"'{selector}' expected '{expected}', got '{actual}'")
        return False

    def log_debug_state(self):
        """Log current UI state for debugging."""
        if self._use_keyboard:
            print("[TauriApp] Debug: keyboard mode, cannot inspect DOM")
            return
        try:
            now_playing = self.get_element_text(".now-playing-status")
            time_selected = self.get_element_text(".time-button.selected .time-label")
            header = self.get_element_text(".content-header h1")
            has_active_btn = self.evaluate_js("!!document.querySelector('.env-button.active')")
            has_active_light = self.evaluate_js(
                "!!document.querySelector('.lighting-preview-widget.active')")
            active_cat = self.evaluate_js(
                "document.querySelector('.category-item.active')"
                "?.closest('[data-category]')?.dataset?.category ?? null")
            print("[TauriApp] === Debug State ===")
            print(f"  Category header: {header}")
            print(f"  Active sidebar category: {active_cat}")
            print(f"  Now playing: {now_playing}")
            print(f"  Selected time: {time_selected}")
            print(f"  Has active env button: {has_active_btn}")
            print(f"  Has active lighting widget: {has_active_light}")
        except Exception as e:
            print(f"[TauriApp] Debug state error: {e}")

    def screenshot(self, path: str):
        """Capture a screenshot of the focused window."""
        # Minimize any inspector windows that may overlay the app
        self._minimize_inspector_windows()

        # Focus the main app window
        if self._window_id:
            subprocess.run(
                ["xdotool", "windowfocus", "--sync", self._window_id],
                check=False,
            )
            subprocess.run(
                ["xdotool", "windowraise", self._window_id],
                check=False,
            )
            time.sleep(0.5)

        # Move mouse cursor to title bar to avoid hover effects on any UI element
        subprocess.run(
            ["xdotool", "mousemove", "--sync", "1", "1"],
            check=False,
        )
        time.sleep(1.5)  # Wait for any tooltips to dismiss
        # Use scrot to capture the full Xvfb display
        result = subprocess.run(
            ["scrot", "--overwrite", path],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            # Fallback to ImageMagick import
            subprocess.run(
                ["import", "-window", "root", path],
                check=True,
            )
        print(f"[TauriApp] Screenshot saved: {path}")

    def _minimize_inspector_windows(self):
        """Minimize any WebKit Inspector windows to prevent overlay in screenshots."""
        result = subprocess.run(
            ["xdotool", "search", "--name", "Web Inspector"],
            capture_output=True, text=True,
        )
        if result.returncode == 0 and result.stdout.strip():
            for wid in result.stdout.strip().split("\n"):
                subprocess.run(["xdotool", "windowminimize", wid], check=False)
                print(f"[TauriApp] Minimized inspector window: {wid}")
        # Also try "Remote Inspector" title variant
        result = subprocess.run(
            ["xdotool", "search", "--name", "Remote Inspector"],
            capture_output=True, text=True,
        )
        if result.returncode == 0 and result.stdout.strip():
            for wid in result.stdout.strip().split("\n"):
                subprocess.run(["xdotool", "windowminimize", wid], check=False)
                print(f"[TauriApp] Minimized inspector window: {wid}")

    def wait(self, seconds: float):
        """Sleep for the given number of seconds."""
        print(f"[TauriApp] Waiting {seconds}s...")
        time.sleep(seconds)

    # =========================================================================
    # Inspector-based UI actions
    # =========================================================================

    def _click_category_inspector(self, name: str):
        """Click a category via JS evaluation."""
        js = f"""
        (() => {{
            const item = document.querySelector('[data-category="{name}"] .category-item');
            if (item) {{ item.click(); return true; }}
            return false;
        }})()
        """
        result = self.evaluate_js(js)
        if not result:
            raise RuntimeError(f"Category '{name}' not found")
        print(f"[TauriApp] Clicked category: {name}")

    def _click_environment_inspector(self, name: str):
        """Click an environment button via JS evaluation."""
        js = f"""
        (() => {{
            const spans = document.querySelectorAll('.env-name');
            for (const span of spans) {{
                if (span.textContent.trim() === '{name}') {{
                    const btn = span.closest('.env-button');
                    if (btn) {{ btn.click(); return true; }}
                }}
            }}
            return false;
        }})()
        """
        result = self.evaluate_js(js)
        if not result:
            raise RuntimeError(f"Environment button '{name}' not found")
        print(f"[TauriApp] Clicked environment: {name}")

    def _select_time_variant_inspector(self, label: str):
        """Select a time variant via JS evaluation."""
        js = f"""
        (() => {{
            const labels = document.querySelectorAll('.time-variant-label');
            for (const lbl of labels) {{
                if (lbl.textContent.trim() === '{label}') {{
                    const btn = lbl.closest('.time-variant-option');
                    if (btn) {{ btn.click(); return true; }}
                }}
            }}
            return false;
        }})()
        """
        result = self.evaluate_js(js)
        if not result:
            raise RuntimeError(f"Time variant '{label}' not found")
        print(f"[TauriApp] Selected time variant: {label}")

    # =========================================================================
    # Keyboard-based UI actions (xdotool fallback)
    # =========================================================================

    # Category order from state.rs ENVIRONMENT_CATEGORIES
    CATEGORY_ORDER = [
        "tavern", "town", "interiors", "travel", "forest", "coastal",
        "dungeon", "combat", "spooky", "relaxation", "celestial",
    ]

    # Shortcut keys for environment buttons (in display order)
    BUTTON_KEYS = "qwertyuiop"

    # Time variant keyboard shortcuts
    TIME_KEYS = {
        "Morning": "1",
        "Daytime": "2",
        "Afternoon": "3",
        "Evening": "4",
    }

    def _click_category_keyboard(self, name: str):
        """Navigate to a category using Ctrl+PgDn."""
        if name not in self.CATEGORY_ORDER:
            raise ValueError(f"Unknown category: {name}")
        target_idx = self.CATEGORY_ORDER.index(name)
        # App starts on first category (index 0)
        for _ in range(target_idx):
            self.send_key("ctrl+Next")  # Ctrl+PgDn
            time.sleep(0.3)
        print(f"[TauriApp] Navigated to category: {name} (keyboard)")

    # Known environment configs per category (sorted alphabetically = display order)
    CATEGORY_ENVS = {
        "travel": [
            "Blizzard", "Boat", "Desert", "River",
            "Snow", "Travel", "Travel Rainy", "Travel Storm",
        ],
    }

    def _click_environment_keyboard(self, name: str, category: str = "travel"):
        """Click an environment button using its shortcut key.

        Environment buttons are sorted alphabetically and assigned Q-P keys.
        """
        envs = self.CATEGORY_ENVS.get(category, [])
        if name in envs:
            idx = envs.index(name)
            key = self.BUTTON_KEYS[idx]
        else:
            # Unknown env - try Q as fallback
            print(f"[TauriApp] Warning: unknown env '{name}' in '{category}', trying Q")
            key = "q"

        self.send_key(key)
        print(f"[TauriApp] Pressed '{key}' for environment: {name} (keyboard)")

    def _select_time_variant_keyboard(self, label: str):
        """Select a time variant using its keyboard shortcut.

        The time variant dialog listens for number keys 1-4:
        1=Morning, 2=Daytime, 3=Afternoon, 4=Evening.
        """
        shortcut_keys = {
            "Morning": "1",
            "Daytime": "2",
            "Afternoon": "3",
            "Evening": "4",
        }

        key = shortcut_keys.get(label)
        if not key:
            raise ValueError(f"Unknown time variant: {label}")

        time.sleep(0.5)
        self.send_key(key)
        print(f"[TauriApp] Sent key '{key}' for time variant '{label}' (keyboard)")

    def send_key(self, key: str):
        """Send a keystroke via xdotool.

        Focus the window first, then send without --window flag.
        Sending with --window can bypass GTK's input routing to the WebView.
        """
        if self._window_id:
            subprocess.run(
                ["xdotool", "windowfocus", "--sync", self._window_id],
                check=False,
            )
            time.sleep(0.1)
        subprocess.run(["xdotool", "key", "--clearmodifiers", key], check=True)

    def send_keys(self, text: str):
        """Type text via xdotool."""
        if self._window_id:
            subprocess.run(
                ["xdotool", "windowfocus", "--sync", self._window_id],
                check=False,
            )
            time.sleep(0.1)
        subprocess.run(["xdotool", "type", "--delay", "50", text], check=True)

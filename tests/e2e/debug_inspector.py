#!/usr/bin/env python3
"""Debug the WebKit Inspector Protocol communication."""

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

BINARY = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.abspath(__file__)))), "rust/target/release/immerse-tauri")
PROJECT_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

print(f"Binary: {BINARY}")

env = os.environ.copy()
# ONLY use HTTP variant - it serves both HTTP listing page AND WebSocket
env["WEBKIT_INSPECTOR_HTTP_SERVER"] = "127.0.0.1:3030"
# Don't set WEBKIT_INSPECTOR_SERVER - it conflicts (WebSocket-only, no HTTP)
env.pop("WEBKIT_INSPECTOR_SERVER", None)
env["RUST_LOG"] = "warn"
env["GTK_A11Y"] = "none"

proc = subprocess.Popen([BINARY], env=env, cwd=PROJECT_DIR,
    stdout=subprocess.PIPE, stderr=subprocess.PIPE)
print(f"PID: {proc.pid}")


def cleanup():
    proc.send_signal(signal.SIGTERM)
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    subprocess.run(["pkill", "-9", "-f", "ffplay"], capture_output=True)
    print("Cleaned up")


print("Waiting 15s for startup...")
time.sleep(15)

# Check process is alive
if proc.poll() is not None:
    print(f"App exited with code {proc.returncode}")
    stderr = proc.stderr.read().decode()
    print(f"Stderr: {stderr[:500]}")
    sys.exit(1)
print("App is running")

# Try HTTP connection
for attempt in range(5):
    try:
        with urllib.request.urlopen("http://127.0.0.1:3030", timeout=5) as resp:
            html = resp.read().decode()
            print(f"HTTP OK ({len(html)} chars)")
            break
    except Exception as e:
        print(f"HTTP attempt {attempt+1}: {e}")
        time.sleep(2)
else:
    print("HTTP failed after 5 attempts")
    cleanup()
    sys.exit(1)

# Find WS path
m = re.search(r"(/socket/\d+/\d+/\w+)", html)
if not m:
    print(f"No socket path. Full HTML:\n{html}")
    cleanup()
    sys.exit(1)

ws_path = m.group(1)
ws_url = f"ws://127.0.0.1:3030{ws_path}"
print(f"WebSocket URL: {ws_url}")

# Connect WebSocket
loop = asyncio.new_event_loop()
try:
    ws = loop.run_until_complete(
        websockets.connect(ws_url, max_size=10*1024*1024, ping_timeout=120)
    )
except Exception as e:
    print(f"WS connect failed: {e}")
    cleanup()
    sys.exit(1)
print("WebSocket connected!")


async def debug_send(msg, label="", timeout=5):
    """Send message and print all responses."""
    print(f"\n--- {label} ---")
    print(f"  Send: {json.dumps(msg)}")
    await ws.send(json.dumps(msg))

    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = await asyncio.wait_for(ws.recv(), timeout=1)
            if isinstance(resp, bytes):
                print(f"  Recv: <binary {len(resp)} bytes>")
            else:
                data = json.loads(resp)
                s = json.dumps(data)
                if len(s) > 500:
                    print(f"  Recv: {s[:500]}...")
                else:
                    print(f"  Recv: {s}")
                if data.get("id") == msg.get("id"):
                    return data
        except asyncio.TimeoutError:
            break
        except Exception as e:
            print(f"  Error: {e}")
            break
    print("  (no matching response)")
    return None


# Test 1: Simple evaluation
r = loop.run_until_complete(debug_send(
    {"id": 1, "method": "Runtime.evaluate",
     "params": {"expression": "42", "returnByValue": True}},
    "Test 1: evaluate '42'"
))

# Test 2: Runtime.enable then evaluate
r = loop.run_until_complete(debug_send(
    {"id": 2, "method": "Runtime.enable"},
    "Test 2a: Runtime.enable"
))

r = loop.run_until_complete(debug_send(
    {"id": 3, "method": "Runtime.evaluate",
     "params": {"expression": "document.title", "returnByValue": True}},
    "Test 2b: document.title"
))

# Test 3: Query DOM
r = loop.run_until_complete(debug_send(
    {"id": 4, "method": "Runtime.evaluate",
     "params": {"expression": "!!document.querySelector('.category-list')", "returnByValue": True}},
    "Test 3: category-list exists?"
))

# Test 4: Get now-playing text
r = loop.run_until_complete(debug_send(
    {"id": 5, "method": "Runtime.evaluate",
     "params": {"expression": "document.querySelector('.now-playing-status')?.textContent", "returnByValue": True}},
    "Test 4: now-playing-status text"
))

# Test 5: Click category and invoke Tauri
r = loop.run_until_complete(debug_send(
    {"id": 6, "method": "Runtime.evaluate",
     "params": {
         "expression": """
         (() => {
             const item = document.querySelector('[data-category="travel"] .category-item');
             if (item) { item.click(); return 'clicked travel'; }
             return 'not found';
         })()
         """,
         "returnByValue": True
     }},
    "Test 5: click travel category"
))

time.sleep(2)

# Test 6: Invoke Tauri IPC
r = loop.run_until_complete(debug_send(
    {"id": 7, "method": "Runtime.evaluate",
     "params": {
         "expression": """
         (async () => {
             try {
                 await window.__TAURI_INTERNALS__.invoke('start_environment_with_time',
                     {configName: 'Travel', time: 'afternoon'});
                 return {success: true};
             } catch (e) {
                 return {success: false, error: String(e)};
             }
         })()
         """,
         "returnByValue": True,
         "awaitPromise": True,
     }},
    "Test 6: Tauri invoke start_environment_with_time",
    timeout=90,
))

time.sleep(3)

# Test 7: Verify state
r = loop.run_until_complete(debug_send(
    {"id": 8, "method": "Runtime.evaluate",
     "params": {
         "expression": """
         JSON.stringify({
             title: document.title,
             nowPlaying: document.querySelector('.now-playing-status')?.textContent,
             timeSelected: document.querySelector('.time-button.selected .time-label')?.textContent,
             header: document.querySelector('.content-header h1')?.textContent,
             hasActiveBtn: !!document.querySelector('.env-button.active'),
             hasActiveLighting: !!document.querySelector('.lighting-preview-widget.active'),
         })
         """,
         "returnByValue": True
     }},
    "Test 7: Full state check",
))

# Take a screenshot
print("\n--- Taking screenshot ---")
screenshot_path = os.path.join(PROJECT_DIR, "tests/e2e/output/debug_screenshot.png")
os.makedirs(os.path.dirname(screenshot_path), exist_ok=True)
subprocess.run(["import", "-window", "root", screenshot_path], capture_output=True, timeout=10)
if os.path.exists(screenshot_path):
    print(f"Screenshot: {screenshot_path} ({os.path.getsize(screenshot_path)} bytes)")
else:
    print("Screenshot failed")

loop.run_until_complete(ws.close())
cleanup()

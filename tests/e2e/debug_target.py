#!/usr/bin/env python3
"""Debug Target.sendMessageToTarget for WebKit Inspector."""

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

BINARY = "~/iye/immerse_yourself/rust/target/release/immerse-tauri"
PROJECT_DIR = "~/iye/immerse_yourself"

env = os.environ.copy()
env["WEBKIT_INSPECTOR_HTTP_SERVER"] = "127.0.0.1:3030"
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

print("Waiting 15s...")
time.sleep(15)

with urllib.request.urlopen("http://127.0.0.1:3030", timeout=5) as resp:
    html = resp.read().decode()
m = re.search(r"(/socket/\d+/\d+/\w+)", html)
ws_url = f"ws://127.0.0.1:3030{m.group(1)}"
print(f"WS: {ws_url}")

loop = asyncio.new_event_loop()
ws = loop.run_until_complete(
    websockets.connect(ws_url, max_size=10*1024*1024, ping_timeout=120)
)
print("Connected!")

target_id = None
outer_id = 0

async def collect_messages(timeout=3):
    """Collect all messages for timeout seconds."""
    global target_id
    msgs = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = await asyncio.wait_for(ws.recv(), timeout=0.5)
            data = json.loads(resp)
            msgs.append(data)
            if data.get("method") == "Target.targetCreated":
                target_id = data["params"]["targetInfo"]["targetId"]
        except asyncio.TimeoutError:
            break
    return msgs

async def send_outer(method, params=None, timeout=5):
    """Send outer-level message."""
    global outer_id
    outer_id += 1
    msg = {"id": outer_id, "method": method}
    if params:
        msg["params"] = params
    await ws.send(json.dumps(msg))

    msgs = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = await asyncio.wait_for(ws.recv(), timeout=1)
            data = json.loads(resp)
            msgs.append(data)
            if data.get("method") == "Target.targetCreated":
                global target_id
                target_id = data["params"]["targetInfo"]["targetId"]
            if data.get("id") == outer_id:
                return data, msgs
        except asyncio.TimeoutError:
            break
    return None, msgs

async def eval_on_target(expression, timeout=10):
    """Evaluate JS on the target page via Target.sendMessageToTarget."""
    global outer_id
    outer_id += 1

    inner_msg = json.dumps({
        "id": outer_id * 1000,
        "method": "Runtime.evaluate",
        "params": {"expression": expression, "returnByValue": True}
    })

    msg = {
        "id": outer_id,
        "method": "Target.sendMessageToTarget",
        "params": {
            "targetId": target_id,
            "message": inner_msg,
        }
    }
    await ws.send(json.dumps(msg))

    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            resp = await asyncio.wait_for(ws.recv(), timeout=1)
            data = json.loads(resp)

            # Check for dispatchMessageFromTarget
            if data.get("method") == "Target.dispatchMessageFromTarget":
                inner = json.loads(data["params"]["message"])
                return inner
            # Check for direct response
            if data.get("id") == outer_id:
                return data
        except asyncio.TimeoutError:
            continue
    return None


# Step 1: Collect initial events
print("\n1. Collecting initial events...")
msgs = loop.run_until_complete(collect_messages(3))
for m2 in msgs:
    print(f"   {json.dumps(m2)[:300]}")
print(f"   Target ID: {target_id}")

if not target_id:
    print("No target found. Trying to list targets...")
    r, msgs = loop.run_until_complete(send_outer("Target.getTargets"))
    for m2 in msgs:
        print(f"   {json.dumps(m2)[:300]}")

if not target_id:
    print("FATAL: No target ID found")
    cleanup()
    sys.exit(1)

# Step 2: Try Runtime.evaluate via Target.sendMessageToTarget
print(f"\n2. Evaluating 'document.title' on target {target_id}...")
result = loop.run_until_complete(eval_on_target("document.title"))
print(f"   Result: {json.dumps(result) if result else 'None'}")

# Step 3: Try DOM query
print("\n3. Checking for .category-list...")
result = loop.run_until_complete(eval_on_target("!!document.querySelector('.category-list')"))
print(f"   Result: {json.dumps(result)[:300] if result else 'None'}")

# Step 4: Get now-playing text
print("\n4. Now playing status...")
result = loop.run_until_complete(eval_on_target(
    "document.querySelector('.now-playing-status')?.textContent?.trim()"))
print(f"   Result: {json.dumps(result)[:300] if result else 'None'}")

# Step 5: Click travel category
print("\n5. Clicking travel category...")
result = loop.run_until_complete(eval_on_target("""
    (() => {
        const item = document.querySelector('[data-category="travel"] .category-item');
        if (item) { item.click(); return 'clicked'; }
        return 'not found';
    })()
"""))
print(f"   Result: {json.dumps(result)[:300] if result else 'None'}")

time.sleep(2)

# Step 6: Invoke Tauri IPC
print("\n6. Invoking start_environment_with_time...")
# For async, we need awaitPromise support - wrap differently
result = loop.run_until_complete(eval_on_target("""
    (async () => {
        try {
            await window.__TAURI_INTERNALS__.invoke('start_environment_with_time',
                {configName: 'Travel', time: 'afternoon'});
            return 'success';
        } catch (e) {
            return 'error: ' + String(e);
        }
    })()
""", timeout=90))
print(f"   Result: {json.dumps(result)[:300] if result else 'None'}")

time.sleep(5)

# Step 7: Verify state
print("\n7. Verifying state...")
result = loop.run_until_complete(eval_on_target("""
    JSON.stringify({
        nowPlaying: document.querySelector('.now-playing-status')?.textContent?.trim(),
        timeSelected: document.querySelector('.time-button.selected .time-label')?.textContent?.trim(),
        header: document.querySelector('.content-header h1')?.textContent?.trim(),
        hasActiveBtn: !!document.querySelector('.env-button.active'),
        hasActiveLighting: !!document.querySelector('.lighting-preview-widget.active'),
    })
"""))
print(f"   Result: {json.dumps(result)[:500] if result else 'None'}")

# Take screenshot
print("\n8. Taking screenshot...")
screenshot_path = "~/iye/immerse_yourself/tests/e2e/output/debug_screenshot.png"
os.makedirs(os.path.dirname(screenshot_path), exist_ok=True)
subprocess.run(["import", "-window", "root", screenshot_path], capture_output=True, timeout=10)
if os.path.exists(screenshot_path):
    print(f"   Saved: {screenshot_path} ({os.path.getsize(screenshot_path)} bytes)")

loop.run_until_complete(ws.close())
cleanup()
print("\nDone!")

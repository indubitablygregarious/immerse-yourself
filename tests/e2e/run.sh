#!/bin/bash
set -e

echo "=== E2E Test Runner ==="

# Start D-Bus session (required for GTK)
export DBUS_SESSION_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address)
echo "[OK] D-Bus started"

# Start PulseAudio with null sink (no real audio device needed)
pulseaudio --start --exit-idle-time=-1 2>/dev/null || true
pactl load-module module-null-sink sink_name=dummy 2>/dev/null || true
pactl set-default-sink dummy 2>/dev/null || true
echo "[OK] PulseAudio started with null sink"

# Start Xvfb at 1920x1080
Xvfb :99 -screen 0 1920x1080x24 -ac +extension GLX +render -noreset &
XVFB_PID=$!
sleep 1

# Verify Xvfb is running
if ! kill -0 $XVFB_PID 2>/dev/null; then
    echo "[FAIL] Xvfb failed to start"
    exit 1
fi
export DISPLAY=:99
echo "[OK] Xvfb started at :99 (1920x1080)"

# Run pytest with all arguments passed through
echo "=== Running tests ==="
cd /app/tests/e2e && python3 -m pytest "$@"
EXIT_CODE=$?

# Cleanup
kill $XVFB_PID 2>/dev/null || true

exit $EXIT_CODE

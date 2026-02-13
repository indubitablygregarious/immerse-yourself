# =============================================================================
# Stage 1: Build the Tauri application
# =============================================================================
FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies: Rust 1.89, Node.js, WebKitGTK dev libs
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Rust 1.89 from Ubuntu repos (same as host)
    cargo-1.89 rustc-1.89 libstd-rust-1.89-dev \
    # Tauri/WebKitGTK build deps
    libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev \
    # Node.js
    nodejs npm \
    # Build essentials
    build-essential pkg-config libssl-dev curl wget \
    # For tauri-cli install
    file \
    && rm -rf /var/lib/apt/lists/*

# Set up Rust 1.89 as default
ENV PATH="/usr/lib/rust-1.89/bin:${PATH}"
ENV RUSTC="/usr/lib/rust-1.89/bin/rustc"

# Install tauri-cli
RUN cargo install tauri-cli --locked

WORKDIR /build

# Copy everything needed for the build
COPY rust/ rust/
COPY Makefile Makefile
COPY env_conf/ env_conf/
COPY sound_conf/ sound_conf/
COPY sounds/ sounds/

# Enable devtools feature so WebKit Inspector HTTP server works in release builds
RUN sed -i 's/tauri = { version = "2", features = \[\] }/tauri = { version = "2", features = ["devtools"] }/' \
    rust/immerse-tauri/Cargo.toml

# Install npm dependencies and build frontend
RUN cd rust/immerse-tauri/ui && npm install && npm run build

# Build the Tauri app
RUN cd rust/immerse-tauri && \
    RUSTC=/usr/lib/rust-1.89/bin/rustc \
    /root/.cargo/bin/cargo-tauri build --no-bundle 2>&1 || \
    RUSTC=/usr/lib/rust-1.89/bin/rustc \
    /root/.cargo/bin/cargo-tauri build 2>&1

# =============================================================================
# Stage 2: Runtime with test tools
# =============================================================================
FROM ubuntu:24.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    # WebKitGTK runtime
    libwebkit2gtk-4.1-0 libgtk-3-0 gir1.2-webkit2-4.1 \
    libjavascriptcoregtk-4.1-0 libsoup-3.0-0 \
    # Display and window management
    xvfb xdotool scrot imagemagick \
    # Audio (null sink for sound engine)
    pulseaudio \
    # D-Bus (required for GTK)
    dbus dbus-x11 \
    # Python for test framework
    python3 python3-pip python3-venv \
    # Fonts for proper UI rendering
    fonts-dejavu-core fonts-liberation fonts-noto-color-emoji \
    # Misc
    procps curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary
COPY --from=builder /build/rust/target/release/immerse-tauri /app/immerse-tauri

# Copy config and sound files
COPY env_conf/ /app/env_conf/
COPY sound_conf/ /app/sound_conf/
COPY sounds/ /app/sounds/

# Copy test files
COPY tests/e2e/requirements.txt /app/tests/e2e/requirements.txt
COPY tests/e2e/*.py /app/tests/e2e/
COPY tests/e2e/run.sh /app/tests/e2e/run.sh
RUN chmod +x /app/tests/e2e/run.sh

# Install Python test dependencies
RUN pip3 install --break-system-packages -r /app/tests/e2e/requirements.txt

# Create output directory
RUN mkdir -p /output

ENTRYPOINT ["/app/tests/e2e/run.sh"]

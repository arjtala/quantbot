#!/usr/bin/env bash
set -euo pipefail

# Install quantbot as a systemd user service.
#
# Usage:
#   bash scripts/install_systemd.sh [--bin PATH] [--config PATH]
#
# Defaults:
#   --bin     ~/.local/bin/quantbot
#   --config  <repo>/config.toml

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN_PATH="$HOME/.local/bin/quantbot"
CONFIG_PATH="$REPO_DIR/config.toml"

while [[ $# -gt 0 ]]; do
    case $1 in
        --bin)    BIN_PATH="$2";    shift 2 ;;
        --config) CONFIG_PATH="$2"; shift 2 ;;
        *)        echo "Unknown option: $1"; exit 1 ;;
    esac
done

QUANTBOT_DIR="$(dirname "$CONFIG_PATH")"

# Ensure binary exists
if [[ ! -f "$BIN_PATH" ]]; then
    RELEASE="$REPO_DIR/target/release/quantbot"
    if [[ -f "$RELEASE" ]]; then
        echo "Copying $RELEASE → $BIN_PATH"
        mkdir -p "$(dirname "$BIN_PATH")"
        cp "$RELEASE" "$BIN_PATH"
        chmod +x "$BIN_PATH"
    else
        echo "ERROR: Binary not found at $BIN_PATH"
        echo "  Run: cargo build --release"
        echo "  Then retry, or pass --bin /path/to/quantbot"
        exit 1
    fi
fi

# Ensure config exists
if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "ERROR: Config not found at $CONFIG_PATH"
    echo "  Copy config.example.toml → config.toml and configure, or pass --config"
    exit 1
fi

# Create env file template if absent
ENV_DIR="$HOME/.config/quantbot"
ENV_FILE="$ENV_DIR/env"
if [[ ! -f "$ENV_FILE" ]]; then
    mkdir -p "$ENV_DIR"
    cat > "$ENV_FILE" << 'ENVEOF'
# IG API credentials — set these for live/demo trading
# IG_API_KEY=
# IG_USERNAME=
# IG_PASSWORD=
ENVEOF
    echo "Created env template: $ENV_FILE"
fi

# Substitute placeholders in service file
SERVICE_DIR="$HOME/.config/systemd/user"
mkdir -p "$SERVICE_DIR"
sed \
    -e "s|QUANTBOT_BIN|$BIN_PATH|g" \
    -e "s|QUANTBOT_DIR|$QUANTBOT_DIR|g" \
    "$REPO_DIR/quantbot.service" > "$SERVICE_DIR/quantbot.service"

echo "Installed: $SERVICE_DIR/quantbot.service"

# Enable linger (allows user services after logout)
if command -v loginctl &>/dev/null; then
    if ! loginctl enable-linger "$(whoami)" 2>/dev/null; then
        echo "WARNING: Could not enable linger. Daemon may stop when you log out."
        echo "  Ask an admin to run: loginctl enable-linger $(whoami)"
    fi
fi

# Reload, enable, start
systemctl --user daemon-reload
systemctl --user enable quantbot.service
systemctl --user start quantbot.service

echo ""
echo "quantbot daemon installed and started."
echo ""
echo "Useful commands:"
echo "  systemctl --user status quantbot"
echo "  journalctl --user -u quantbot -f"
echo "  systemctl --user restart quantbot"
echo "  systemctl --user stop quantbot"
echo ""
systemctl --user status quantbot --no-pager || true

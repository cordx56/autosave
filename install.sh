#!/bin/bash -e

case "$(uname -s)" in
    Linux)
        HOST_OS="unknown-linux-gnu"
        ;;
    Darwin)
        HOST_OS="apple-darwin"
        ;;
    *)
        echo "unsupported OS"
        return 1
        ;;
esac

case "$(uname -m)" in
    arm64|aarch64)
        HOST_ARCH="aarch64"
        ;;
    x86_64|amd64)
        HOST_ARCH="aarch64"
        ;;
    *)
        echo "unsupported architecture"
        return 1
        ;;
esac

HOST_TUPLE="$HOST_ARCH-$HOST_OS"
CRATE_NAME="autosave"

if [ -n "$CARGO_HOME" ]; then
    BIN_PATH="$CARGO_HOME/bin"
elif echo ":$PATH:" | grep -q ":$HOME/.cargo/bin:"; then
    BIN_PATH="$HOME/.cargo/bin"
else
    BIN_PATH="$HOME/.local/bin"
fi
curl -L "https://github.com/cordx56/autosave/releases/latest/download/$CRATE_NAME-$HOST_TUPLE" > "$BIN_PATH/$CRATE_NAME"

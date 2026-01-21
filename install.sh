#!/bin/bash -e

case "$(uname -s)" in
    Linux)
        HOST_OS="unknown-linux-gnu"
        OS_LIB="so"
        ;;
    Darwin)
        HOST_OS="apple-darwin"
        OS_LIB="dylib"
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
LIB_NAME="lib$CRATE_NAME.$OS_LIB"

if which cargo > /dev/null; then
    if [ -n "$CARGO_HOME" ]; then
        BIN_PATH="$CARGO_HOME/bin"
    elif echo ":$PATH:" | grep -q ":$HOME/.cargo/bin:"; then
        BIN_PATH="$HOME/.cargo/bin"
    else
        BIN_PATH="$HOME/.local/bin"
    fi

    if ls Cargo.toml > /dev/null; then
        cargo build --release --locked

        cp "target/release/$LIB_NAME" "$BIN_PATH/"
        cp "target/release/$CRATE_NAME" "$BIN_PATH/"
    fi
else
    BIN_PATH="$HOME/.local/bin"
    curl -L "https://github.com/cordx56/autosave/releases/latest/download/$LIB_NAME" > "$BIN_PATH/$LIB_NAME"
    curl -L "https://github.com/cordx56/autosave/releases/latest/download/$CRATE_NAME" > "$BIN_PATH/$CRATE_NAME"
fi

#!/usr/bin/env bash
# Install Professor X as a global `profx` command — so you launch it like any
# other harness: just type `profx` in any folder (it'll ask to trust the folder,
# then open the assistant session).
#
#   ./install.sh
#   profx                      # opens the session in the current folder
#   profx --task "..."         # one-shot
#   profx --model qwen3:14b    # pick a local model (else biggest installed)
set -euo pipefail

cd "$(dirname "$0")"
BIN_DIR="${HOME}/.local/bin"
TARGET="$(pwd)/target/release/professor-x"

echo "Building Professor X (release)…"
cargo build --release

mkdir -p "$BIN_DIR"
ln -sf "$TARGET" "$BIN_DIR/profx"
echo "Linked: $BIN_DIR/profx -> $TARGET"

if ! command -v profx >/dev/null 2>&1; then
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
      echo
      echo "⚠  $BIN_DIR is not on your PATH. Add this to your shell rc:"
      echo "     export PATH=\"\$HOME/.local/bin:\$PATH\""
      ;;
  esac
fi

echo
echo "Done. Make sure Ollama is running and you've pulled a model:"
echo "     ollama pull qwen3:8b-q4_K_M"
echo
echo "Then, from any folder:   profx"

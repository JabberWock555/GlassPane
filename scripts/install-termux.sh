#!/data/data/com.termux/files/usr/bin/bash
# Build glasspane on an Android phone and run it as a Termux service (runit),
# next to Home Assistant. Needs the termux-services package (sv, sv-enable).
# Run from the project folder:  bash scripts/install-termux.sh
set -euo pipefail
cd "$(dirname "$0")/.."
APP_DIR="$PWD"

echo "==> Installing Rust, curl and termux-services (one-time)"
pkg install -y rust curl termux-services >/dev/null

echo "==> Building release binary (first build takes ~10 min on the phone)"
cargo build --release --bin glasspane

if [ ! -f glasspane.toml ]; then
  cp glasspane.example.toml glasspane.toml
  echo "==> Created glasspane.toml from the example; edit the entity ids."
fi

ENV_FILE="$HOME/.config/glasspane/env"
if [ ! -f "$ENV_FILE" ]; then
  mkdir -p "$(dirname "$ENV_FILE")"
  echo 'HA_TOKEN=paste-your-long-lived-token-here' > "$ENV_FILE"
  chmod 600 "$ENV_FILE"
  echo "==> Put your Home Assistant long-lived token in $ENV_FILE"
fi

SV="$PREFIX/var/service/glasspane"
mkdir -p "$SV/log"
ln -sf "$PREFIX/share/termux-services/svlogger" "$SV/log/run"
cat > "$SV/run" <<EOF
#!/data/data/com.termux/files/usr/bin/sh
exec 2>&1
cd "$APP_DIR"
set -a; . "$ENV_FILE"; set +a
exec ./target/release/glasspane glasspane.toml
EOF
chmod +x "$SV/run"

sv-enable glasspane 2>/dev/null || true
sv restart glasspane
mkdir -p /sdcard/Pictures/Glasspane 2>/dev/null || true  # screensaver photos
echo "==> Done. Status: $(sv status glasspane)"
echo "    Open http://<phone-ip>:8090 on the tablet. Logs: tail -f \$PREFIX/var/log/sv/glasspane/current"

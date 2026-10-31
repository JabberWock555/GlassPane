#!/data/data/com.termux/files/usr/bin/bash
# Update glasspane on an Android phone (Termux) and restart the service.
#
#   bash ~/glasspane/scripts/update-termux.sh                 # git checkout: git pull
#   bash ~/glasspane/scripts/update-termux.sh update.zip      # from a zip of the repo
#   bash ~/glasspane/scripts/update-termux.sh update.zip --config my.toml
#
# Your glasspane.toml is always kept (a backup goes to ~/glasspane.toml.bak).
# --config FILE replaces it with FILE (a path inside the zip or on the phone).
set -euo pipefail

# The update replaces this very file, and bash reads scripts as it runs them.
# Re-run from a copy so the update can't change the script mid-execution.
if [ -z "${GLASSPANE_UPDATER:-}" ]; then
  cp "$0" "$HOME/.glasspane-updater.sh"
  GLASSPANE_UPDATER=1 exec bash "$HOME/.glasspane-updater.sh" "$@"
fi

DST="$HOME/glasspane"
ZIP=""
CONFIG=""
while [ $# -gt 0 ]; do
  case "$1" in
    --config) CONFIG="$2"; shift 2 ;;
    *) ZIP="$1"; shift ;;
  esac
done

cp "$DST/glasspane.toml" "$HOME/glasspane.toml.bak" 2>/dev/null || true

if [ -z "$ZIP" ] && [ -d "$DST/.git" ]; then
  echo "==> git pull"
  git -C "$DST" pull --ff-only
else
  ZIP="${ZIP:-/sdcard/Download/glasspane-update.zip}"
  echo "==> Unpacking $ZIP"
  command -v unzip >/dev/null || pkg install -y unzip >/dev/null
  TMP="$HOME/.glasspane-update"
  rm -rf "$TMP" && mkdir -p "$TMP"
  unzip -q "$ZIP" -d "$TMP"
  SRC="$TMP/glasspane"
  # Replace code, keep runtime data (settings.json, usage.json, .photo-cache).
  rm -rf "$DST/src" "$DST/web/dist" "$DST/web/src" "$DST/web/static"
  cp -r "$SRC/src" "$SRC/scripts" "$SRC/tests" "$SRC/docs" "$DST/" 2>/dev/null || true
  cp -r "$SRC/web/dist" "$SRC/web/src" "$SRC/web/static" "$DST/web/"
  cp "$SRC"/Cargo.toml "$SRC"/Cargo.lock "$SRC"/*.toml "$SRC"/*.md "$DST/"
  if [ -n "$CONFIG" ] && [ -f "$SRC/$CONFIG" ]; then CONFIG="$SRC/$CONFIG"; fi
fi

if [ -n "$CONFIG" ]; then
  cp "$CONFIG" "$DST/glasspane.toml"
elif [ -f "$HOME/glasspane.toml.bak" ]; then
  cp "$HOME/glasspane.toml.bak" "$DST/glasspane.toml"
fi

cd "$DST"
echo "==> Building (incremental, ~1-2 min on a phone)"
cargo build --release --bin glasspane
sv restart glasspane
sleep 3
echo "==> $(sv status glasspane)"
rm -rf "$HOME/.glasspane-update"

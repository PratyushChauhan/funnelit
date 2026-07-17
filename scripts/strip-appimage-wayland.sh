#!/usr/bin/env bash
# Remove bundled libwayland* from a Tauri AppImage and repack it.
# Host Wayland/mesa then load instead of the older Ubuntu CI copies that
# trigger WebKit EGL_BAD_PARAMETER on modern Wayland sessions.
set -euo pipefail

BUNDLE_DIR="${1:-src-tauri/target/release/bundle/appimage}"
if [[ ! -d "$BUNDLE_DIR" ]]; then
  echo "Bundle dir $BUNDLE_DIR not found; nothing to strip."
  exit 0
fi

cd "$BUNDLE_DIR"
APPIMAGE="$(ls -1 ./*.AppImage 2>/dev/null | head -n1 || true)"
if [[ -z "$APPIMAGE" ]]; then
  echo "No AppImage found in $BUNDLE_DIR; nothing to strip."
  exit 0
fi

echo "Stripping bundled Wayland libs from $APPIMAGE"
rm -rf squashfs-root
./"$APPIMAGE" --appimage-extract >/dev/null
rm -f "$APPIMAGE"

removed=0
while IFS= read -r -d '' lib; do
  echo "  removing $lib"
  rm -f "$lib"
  removed=$((removed + 1))
done < <(find squashfs-root -type f \( \
  -name 'libwayland-client.so*' -o \
  -name 'libwayland-egl.so*' -o \
  -name 'libwayland-cursor.so*' -o \
  -name 'libwayland-server.so*' \
\) -print0)

echo "Removed $removed bundled Wayland libraries."

case "$(uname -m)" in
  x86_64) AT_ARCH=x86_64 ;;
  aarch64 | arm64) AT_ARCH=aarch64 ;;
  *) AT_ARCH="$(uname -m)" ;;
esac

TOOL="appimagetool-${AT_ARCH}.AppImage"
if [[ ! -x "$TOOL" ]]; then
  curl -fsSL \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${AT_ARCH}.AppImage" \
    -o "$TOOL"
  chmod +x "$TOOL"
fi

ARCH="$AT_ARCH" APPIMAGE_EXTRACT_AND_RUN=1 ./"$TOOL" squashfs-root "$APPIMAGE"
rm -rf squashfs-root
echo "Repacked $APPIMAGE without bundled Wayland libraries."

#!/bin/bash
# sssssssssampler installer — run this once after unzipping
set -e

DIR="$(cd "$(dirname "$0")" && pwd)"
VST3="$DIR/sssssssssampler.vst3"
CLAP="$DIR/sssssssssampler.clap"
VST3_DEST="$HOME/Library/Audio/Plug-Ins/VST3"
CLAP_DEST="$HOME/Library/Audio/Plug-Ins/CLAP"

echo "sssssssssampler installer"
echo "─────────────────────────"

# Check bundles exist
if [ ! -d "$VST3" ] && [ ! -d "$CLAP" ]; then
    echo "✗ No plugin bundles found next to this script."
    echo "  Make sure install.sh is in the same folder as the .vst3 and .clap files."
    exit 1
fi

# Create plugin dirs if needed
mkdir -p "$VST3_DEST" "$CLAP_DEST"

# Install
if [ -d "$VST3" ]; then
    echo "→ installing VST3..."
    cp -r "$VST3" "$VST3_DEST/"
fi
if [ -d "$CLAP" ]; then
    echo "→ installing CLAP..."
    cp -r "$CLAP" "$CLAP_DEST/"
fi

# Clear quarantine (macOS marks downloaded files as untrusted)
echo "→ clearing quarantine..."
xattr -rd com.apple.quarantine "$VST3_DEST/sssssssssampler.vst3" 2>/dev/null || true
xattr -rd com.apple.quarantine "$CLAP_DEST/sssssssssampler.clap"  2>/dev/null || true

# Ad-hoc codesign (no Apple dev account needed)
echo "→ signing..."
codesign -s - --force --deep "$VST3_DEST/sssssssssampler.vst3"
codesign -s - --force --deep "$CLAP_DEST/sssssssssampler.clap"

# Clear Ableton plugin cache for any installed Live version
echo "→ clearing Ableton plugin cache..."
for db in "$HOME/Library/Preferences/Ableton/Live "*/PluginScanDb.txt; do
    [ -f "$db" ] && rm -f "$db" && echo "  cleared: $db"
done

echo ""
echo "✓ done — relaunch Ableton (or your DAW) to scan for the plugin"

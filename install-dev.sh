#!/bin/bash
set -e
cd "$(dirname "$0")"

echo "→ bundling (dev)..."
cargo xtask bundle sssssssssampler

echo "→ installing..."
cp -r target/bundled/sssssssssampler.vst3 ~/Library/Audio/Plug-Ins/VST3/
cp -r target/bundled/sssssssssampler.clap ~/Library/Audio/Plug-Ins/CLAP/

echo "→ clearing quarantine + signing..."
xattr -rd com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/sssssssssampler.vst3 2>/dev/null || true
xattr -rd com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/sssssssssampler.clap 2>/dev/null || true
codesign -s - --force --deep ~/Library/Audio/Plug-Ins/VST3/sssssssssampler.vst3
codesign -s - --force --deep ~/Library/Audio/Plug-Ins/CLAP/sssssssssampler.clap

echo "→ clearing Ableton plugin cache..."
rm -f "$HOME/Library/Preferences/Ableton/Live 12.3.6/PluginScanDb.txt"

echo "✓ done — relaunch Ableton"

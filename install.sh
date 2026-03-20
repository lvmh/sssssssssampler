#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "→ bundling..."
cargo xtask bundle sssssssssampler --release

echo "→ installing..."
cp -r target/bundled/sssssssssampler.vst3 ~/Library/Audio/Plug-Ins/VST3/
cp -r target/bundled/sssssssssampler.clap ~/Library/Audio/Plug-Ins/CLAP/

echo "→ clearing quarantine..."
xattr -rd com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/sssssssssampler.vst3
xattr -rd com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/sssssssssampler.clap

echo "→ signing..."
codesign -s - --force --deep ~/Library/Audio/Plug-Ins/VST3/sssssssssampler.vst3
codesign -s - --force --deep ~/Library/Audio/Plug-Ins/CLAP/sssssssssampler.clap

echo "→ clearing Ableton plugin cache..."
rm -f "$HOME/Library/Preferences/Ableton/Live 12.3.6/PluginScanDb.txt"

echo "✓ done — relaunch Ableton"

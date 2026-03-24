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
for db in "$HOME/Library/Preferences/Ableton/Live "*/PluginScanDb.txt; do
    [ -f "$db" ] && rm -f "$db"
done

# Copy the standalone installer into the bundle folder so it's included in release zips
cp installer.sh target/bundled/install.sh

echo "✓ done — relaunch Ableton"

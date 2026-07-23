#!/bin/bash
pkill -f "target/debug/dualsense-haptics" 2>/dev/null
pkill -f "tauri dev" 2>/dev/null
pkill -f "Universal DualSense" 2>/dev/null
sleep 1
cd "$(dirname "$0")"
npm run dev

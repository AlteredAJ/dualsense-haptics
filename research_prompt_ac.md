# Gemini Research Prompt — Assetto Corsa Telemetry

## Context
The app currently parses Forza Horizon Data Out UDP telemetry and has a design for F1 23 UDP telemetry. We need the same for Assetto Corsa (original, not Competizione unless that's also relevant) to add it as a `GameSource`.

## Research Questions

### 1. Telemetry Availability
- Does Assetto Corsa broadcast UDP telemetry natively (like Forza / F1)?
- Is there a built-in "Data Out" or "Telemetry" settings page in the UI?
- If not, does it expose telemetry through shared memory, a plugin API, or a third-party app like SimHub?

### 2. Network Configuration
- What UDP port(s) does Assetto Corsa use for telemetry?
- Does it support configurable ports and IP targets?
- Can the broadcast be restricted to localhost (127.0.0.1)?

### 3. Packet Format
- What is the binary format? Is it a flat C-struct like Forza's Dash V2, or a multi-packet header-based format like F1 23?
- Is there a packet header with an identifier/version field?
- What is the endianness (little-endian like Forza/F1)?
- What is the total packet size in bytes?

### 4. Wheel Order Convention
- What is the wheel array index order? Common conventions:
  - Forza: 0=FL, 1=FR, 2=RL, 3=RR
  - F1 23: 0=RL, 1=RR, 2=FL, 3=FR
  - Some sims: 0=FL, 1=FR, 2=RL, 3=RR (same as Forza)
- What does Assetto Corsa use?

### 5. Key Telemetry Fields (with byte offsets if known)
- Engine RPM
- Vehicle speed (m/s or km/h?)
- Per-wheel tire slip ratio (longitudinal)
- Per-wheel tire slip angle (lateral) — if available
- Per-wheel suspension travel (normalized?)
- Surface/road texture or kerb contact
- Throttle and brake pedal input (0-1 or 0-255?)
- Current gear
- Longitudinal/lateral/vertical acceleration (G-forces)

### 6. Compatibility Notes
- Are there differences between Assetto Corsa (original) and Assetto Corsa Competizione?
- Does the format change with game version or mods (Content Manager, CSP)?
- Are there known community projects or libraries that already parse this format?

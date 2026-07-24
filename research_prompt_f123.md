# Gemini Research Prompt — F1 23 UDP Telemetry

## Context
The app currently parses Forza Horizon Data Out UDP telemetry. We need to add F1 23 as a GameSource. Gemini has already confirmed F1 23 uses a multi-packet header-based UDP format on port 20777 with little-endian byte ordering and RL-RR-FL-FR wheel order. This prompt asks for the specific byte layouts needed to implement the parser.

## Research Questions

### 1. PacketHeader (29 bytes)
What is the exact layout of the 29-byte PacketHeader? Need offsets and types for:
- m_packetFormat (uint16)
- m_gameYear (uint8)
- m_packetId (uint8) — the routing field
- m_sessionUID (uint64)
- m_sessionTime (float32)
- m_frameIdentifier (uint32)
- m_playerCarIndex (uint8)
- All other header fields with their exact byte offsets

### 2. PacketCarTelemetryData (ID 6) — 1352 bytes
The player's car data is at index m_playerCarIndex in the carTelemetryData array. Need offsets for each field in a single CarTelemetryData struct:
- m_speed (uint16) — in km/h
- m_throttle (float32) — 0.0 to 1.0
- m_brake (float32) — 0.0 to 1.0
- m_engineRPM (uint16)
- m_gear (int8) — -1 = reverse, 0 = neutral, 1-8 = gears
- m_surfaceType (uint8[4]) — per-wheel: 0=tarmac, 1=rumble strip, etc.
- Any other fields in the struct with offsets

### 3. PacketMotionExData (ID 13) — 217 bytes
Per-wheel arrays use RL-RR-FL-FR order. Need field offsets and types:
- m_suspensionPosition (float32[4]) — normalized 0-1?
- m_suspensionVelocity (float32[4])
- m_wheelSlipRatio (float32[4]) — longitudinal
- m_wheelSlipAngle (float32[4]) — lateral
- m_wheelSpeed (float32[4])
- Any other motion fields with offsets

### 4. Packet Format Notes
- Does F1 23 broadcast ALL packet types every frame, or does it rotate?
- What interval does each packet type arrive at?
- Are there any frame-counter or timestamp fields for detecting dropped packets?
- Does the game need to be in a specific mode (on-track, not paused) for telemetry to broadcast?

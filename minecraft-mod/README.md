# DualSense Haptics Bridge (Fabric, Minecraft 1.20.1)

Phase 1 of the Minecraft profile. This client-side Fabric mod streams the
category of the item in your main hand to the Universal DualSense Haptics app
over a localhost TCP socket (`127.0.0.1:27812`). The app maps each category to a
lightbar color, so switching items in-game recolors the controller. Per-item
trigger/rumble feels come in Phase 2.

## Wire format

One JSON object per line, sent only when the held item's category changes:

```
{"item":"sword"}
{"item":"pickaxe"}
{"item":"empty"}
```

Categories: `empty`, `sword`, `axe`, `pickaxe`, `shovel`, `hoe`, `bow`,
`crossbow`, `trident`, `shield`, `food`, `block`, `other`.

## Building

This machine has no JDK/Gradle, so build it in your Fabric dev environment:

1. Install **JDK 17** (Temurin/Adoptium works well).
2. Open this `minecraft-mod/` folder in IntelliJ IDEA (it generates the Gradle
   wrapper on import), or from a terminal with Gradle installed:
   ```
   gradle wrapper        # one-time: creates ./gradlew
   ./gradlew build
   ```
3. The built mod jar lands in `build/libs/dualsense-haptics-bridge-0.3.0.jar`.
4. Drop that jar into your `.minecraft/mods/` folder alongside **Fabric API**
   for 1.20.1, with the Fabric loader profile selected.

## Testing Phase 1

1. Launch the haptics app (`npm run dev`) and select the **Minecraft** profile.
   The UI shows "Mod not connected".
2. Launch Minecraft 1.20.1 (Fabric) with this mod + Fabric API installed.
3. Join any world. The app should flip to "Mod connected" and the held-item
   field should update as you scroll the hotbar. The controller lightbar
   recolors per item (sword = steel, pickaxe = gray, food = red, etc.).

If the app isn't running, the mod retries the connection every 2 seconds, so
order of startup doesn't matter.

## Forge

A Forge port can reuse the same `senderLoop` socket logic with a Forge
`ClientTickEvent` hook. A loader toggle in the app is planned for later.

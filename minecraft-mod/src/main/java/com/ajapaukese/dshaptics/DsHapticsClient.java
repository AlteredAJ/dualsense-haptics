package com.ajapaukese.dshaptics;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.item.AxeItem;
import net.minecraft.item.BlockItem;
import net.minecraft.item.BowItem;
import net.minecraft.item.CrossbowItem;
import net.minecraft.item.HoeItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.PickaxeItem;
import net.minecraft.item.ShieldItem;
import net.minecraft.item.ShovelItem;
import net.minecraft.item.SwordItem;
import net.minecraft.item.TridentItem;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * Streams gameplay state to the Universal DualSense Haptics app over a localhost
 * TCP socket as newline-delimited JSON, one line per client tick (~20 Hz):
 *
 *   {"item":"bow","using":true,"useProg":0.73,"mining":false,"blocking":false,
 *    "attack":false,"hurt":false,"sprinting":true,"onGround":true,"health":14.0}
 *
 * "attack" and "hurt" are rising-edge one-shots (true for the single tick the
 * event starts). The app maps all of this to per-item adaptive-trigger and
 * rumble feels. The app is the server; this mod reconnects on its own.
 */
public class DsHapticsClient implements ClientModInitializer {
    private static final Logger LOG = LoggerFactory.getLogger("ds-haptics-bridge");
    private static final String HOST = "127.0.0.1";
    private static final int PORT = 27812;

    // Tick thread enqueues lines; the sender thread drains and writes them.
    // Capped so a closed socket can't grow it without bound.
    private final ConcurrentLinkedQueue<String> queue = new ConcurrentLinkedQueue<>();
    private static final int QUEUE_CAP = 64;

    // Rising-edge tracking (tick thread only).
    private boolean wasSwinging = false;
    private int prevHurtTime = 0;

    @Override
    public void onInitializeClient() {
        Thread sender = new Thread(this::senderLoop, "ds-haptics-bridge");
        sender.setDaemon(true);
        sender.start();

        ClientTickEvents.END_CLIENT_TICK.register(this::onTick);
        LOG.info("DualSense Haptics Bridge initialized — streaming to {}:{}", HOST, PORT);
    }

    private void onTick(MinecraftClient client) {
        ClientPlayerEntity p = client.player;
        if (p == null) {
            enqueue("{\"item\":\"empty\"}\n");
            wasSwinging = false;
            prevHurtTime = 0;
            return;
        }

        ItemStack stack = p.getMainHandStack();
        String item = categorize(stack);

        boolean using = p.isUsingItem();
        float useProg = useProgress(p, item, using);
        boolean mining = client.interactionManager != null && client.interactionManager.isBreakingBlock();
        boolean blocking = p.isBlocking();
        boolean sprinting = p.isSprinting();
        boolean onGround = p.isOnGround();
        float health = p.getHealth();

        // Rising-edge swing → attack event.
        boolean swinging = p.handSwinging;
        boolean attack = swinging && !wasSwinging;
        wasSwinging = swinging;

        // Rising-edge hurt: hurtTime jumps to its max the tick damage lands.
        boolean hurt = p.hurtTime > prevHurtTime && prevHurtTime == 0;
        prevHurtTime = p.hurtTime;

        enqueue(String.format(Locale.US,
            "{\"item\":\"%s\",\"using\":%b,\"useProg\":%.2f,\"mining\":%b,\"blocking\":%b,"
                + "\"attack\":%b,\"hurt\":%b,\"sprinting\":%b,\"onGround\":%b,\"health\":%.1f}\n",
            item, using, useProg, mining, blocking, attack, hurt, sprinting, onGround, health));
    }

    private float useProgress(ClientPlayerEntity p, String item, boolean using) {
        if (!using) {
            return 0f;
        }
        ItemStack active = p.getActiveItem();
        int max = active.getMaxUseTime();
        int used = max - p.getItemUseTimeLeft();
        if (item.equals("bow")) {
            return BowItem.getPullProgress(used);
        }
        // Crossbow charge, trident, food, potion — normalize against the use time
        // (capped so longer-use items still ramp meaningfully).
        int span = Math.min(max <= 0 ? 30 : max, 30);
        return Math.max(0f, Math.min(1f, used / (float) span));
    }

    private String categorize(ItemStack stack) {
        if (stack.isEmpty()) {
            return "empty";
        }
        Item item = stack.getItem();
        if (item instanceof SwordItem)    return "sword";
        if (item instanceof AxeItem)      return "axe";
        if (item instanceof PickaxeItem)  return "pickaxe";
        if (item instanceof ShovelItem)   return "shovel";
        if (item instanceof HoeItem)      return "hoe";
        if (item instanceof BowItem)      return "bow";
        if (item instanceof CrossbowItem) return "crossbow";
        if (item instanceof TridentItem)  return "trident";
        if (item instanceof ShieldItem)   return "shield";
        if (item.isFood())                return "food";
        if (item instanceof BlockItem)    return "block";
        return "other";
    }

    private void enqueue(String line) {
        if (queue.size() >= QUEUE_CAP) {
            queue.poll(); // drop oldest — app is down or slow
        }
        queue.add(line);
    }

    private void senderLoop() {
        while (true) {
            try (Socket socket = new Socket()) {
                socket.connect(new InetSocketAddress(HOST, PORT), 2000);
                socket.setTcpNoDelay(true);
                OutputStream out = socket.getOutputStream();
                LOG.info("Connected to haptics app at {}:{}", HOST, PORT);
                queue.clear(); // drop anything stale that piled up while disconnected

                while (!socket.isClosed()) {
                    String line = queue.poll();
                    if (line == null) {
                        Thread.sleep(8);
                        continue;
                    }
                    out.write(line.getBytes(StandardCharsets.UTF_8));
                    out.flush();
                }
            } catch (InterruptedException ie) {
                return;
            } catch (Exception e) {
                try {
                    Thread.sleep(2000); // app not running / dropped — back off
                } catch (InterruptedException ie) {
                    return;
                }
            }
        }
    }
}

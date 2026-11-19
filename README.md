# Socrate forked duh, only did this cuz u made it paid

Tauri 2 + SvelteKit overlay. Same external pipeline as Cherry, rebuilt from scratch around a modular Rust backend.

## Structure

```
src/                       SvelteKit overlay (empty black menu placeholder)
src-tauri/
  src/
    main.rs                bin entry
    lib.rs                 wires app state + commands + threads
    offsets.rs             Roblox memory offsets (current version)
    math.rs                view / projection / world_to_ndc
    sdk/                   process attach, vm_read / vm_write, instance walk, pattern scan
    overlay/               NSWindow setup, click-through tracking, CGWindow bounds polling
  capabilities/            Tauri capability manifests
  Info.plist               macOS bundle Info.plist
  entitlements.plist       macOS entitlements (debugger, get-task-allow)
```

## Dev

```bash
npm install
npm run tauri dev
```

Disclaimer This project is intended strictly for educational and research purposes only. It is not designed, intended, or endorsed for use in any form of cheating, hacking, or violation of a game’s Terms of Service. The author assumes no responsibility for how this code is used by others. Any misuse is solely the responsibility of the end user. By using or modifying this code, you agree that you understand and accept these terms.

//! Roblox memory offsets — version-pinned table.
//!
//! Bump this file every time Roblox auto-updates. The version string below
//! comes from the macOS Roblox build (`version-XXXX...`) — keep it in sync
//! so we can detect a mismatched binary at runtime.

#![allow(dead_code)]

/// The Roblox build this offset table was dumped against.
pub const ROBLOX_VERSION: &str = "version-73d88b9483b340ae";

// ─── Entry — static offsets relative to the Roblox base image ─────────────
pub enum Entry {
    /// `Roblox.base + FakeDataModel` → pointer to the FakeDataModel struct.
    FakeDataModel = 0x5FB7D40,
}

// ─── Instance (shared layout for every Roblox Instance) ──────────────────
pub enum Instance {
    Parent = 0x070,
    ChildrenList = 0x078,
    Name = 0x0B0,
}

// ─── FakeDataModel → DataModel ───────────────────────────────────────────
pub enum FakeDataModel {
    DataModel = 0x18,
}

// ─── DataModel — services ────────────────────────────────────────────────
pub enum DataModel {
    WorkSpace = 0x168,
    ContextActionService = 0x358,
    StarterPack = 0x368,
    StarterGui = 0x378,
    StarterPlayer = 0x388,
    RunService = 0x3A8,
    UserInputService = 0x3B8,
}

// ─── Workspace ───────────────────────────────────────────────────────────
pub enum Workspace {
    Terrain = 0x470,
    Camera = 0x460,
}

// ─── Camera ──────────────────────────────────────────────────────────────
pub enum Camera {
    /// 3×3 float matrix (CFrame rotation). Column 0 = Right, 1 = Up, 2 = Back.
    CameraOrientation = 0xF0,
    /// Vec3 f32 — camera world position.
    CameraPosition = 0x1C0,
    /// Vec3 f32 — point the camera is looking at.
    CameraFocusPosition = 0x1F0,
}

// ─── Humanoid ────────────────────────────────────────────────────────────
pub enum Humanoid {
    Health = 0x184,
}

// ─── HumanoidRootPart (and any BasePart-like instance) ───────────────────
pub enum HumanoidRootPart {
    /// Pointer to the Primitive struct (where the real Vec3 position lives).
    Primitive = 0x138,
    /// Vec3 f32 — fallback world position read directly on the part instance.
    Position = 0x134,
}

/// Generic "BasePart" view — every visible part exposes its Primitive at the
/// same offset, so the ESP code can use this regardless of part name.
pub enum BasePart {
    Primitive = 0x138,
}

/// Primitive struct — holds the canonical Vec3 the renderer should use.
/// `Position` is the f32x3 at the address `BasePart.Primitive` points to.
pub enum Primitive {
    /// Matches `HumanoidRootPart::Position` (0x134) — same shape, but the
    /// pointer-chain version (BasePart -> Primitive -> +0x134) is the one
    /// the ESP loop uses for non-HRP parts.
    Position = 0x134,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    Mouse,
    Keyboard,
}

#[derive(Clone, Debug)]
pub struct LegionModel {
    pub id: &'static str,
    pub name: &'static str,
    pub pids: &'static [u16],
    pub interface: u8,
    pub kind: DeviceKind,
    pub max_dpi: u32,
    pub max_stages: usize,
    pub buttons: usize,
}

pub const VID_LENOVO: u16 = 0x17EF;

pub const SUPPORTED_MODELS: &[LegionModel] = &[
    LegionModel {
        id: "m300",
        name: "Legion M300 RGB Gaming Mouse",
        pids: &[0x60E4],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 8000,
        max_stages: 3,
        buttons: 8,
    },
    LegionModel {
        id: "m300s",
        name: "Legion M300s RGB Gaming Mouse",
        pids: &[0x6132, 0x6155],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 8000,
        max_stages: 4,
        buttons: 6,
    },
    LegionModel {
        id: "m500",
        name: "Legion M500 RGB Gaming Mouse",
        pids: &[0x60D4],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 16000,
        max_stages: 5,
        buttons: 7,
    },
    LegionModel {
        id: "m600",
        name: "Legion M600 Wireless Gaming Mouse",
        pids: &[0x60E5, 0x60E6],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 16000,
        max_stages: 4,
        buttons: 8,
    },
    LegionModel {
        id: "m600s",
        name: "Legion M600s Wireless Gaming Mouse",
        pids: &[0x6133, 0x6135, 0x6140],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 19000,
        max_stages: 4,
        buttons: 6,
    },
    LegionModel {
        id: "m600sqi",
        name: "Legion M600s Qi Wireless Mouse",
        pids: &[0x6136, 0x6138, 0x6141],
        interface: 2,
        kind: DeviceKind::Mouse,
        max_dpi: 19000,
        max_stages: 4,
        buttons: 6,
    },
    LegionModel {
        id: "k300",
        name: "Legion K300 RGB Gaming Keyboard",
        pids: &[0x60F0],
        interface: 3,
        kind: DeviceKind::Keyboard,
        max_dpi: 0,
        max_stages: 0,
        buttons: 0,
    },
    LegionModel {
        id: "k310",
        name: "Legion K310 RGB Gaming Keyboard",
        pids: &[0x6199],
        interface: 3,
        kind: DeviceKind::Keyboard,
        max_dpi: 0,
        max_stages: 0,
        buttons: 0,
    },
    LegionModel {
        id: "k310ai",
        name: "Legion K310 AI RGB Keyboard",
        pids: &[0x6205],
        interface: 3,
        kind: DeviceKind::Keyboard,
        max_dpi: 0,
        max_stages: 0,
        buttons: 0,
    },
    LegionModel {
        id: "k500",
        name: "Legion K500 Mechanical Keyboard",
        pids: &[0x60D5],
        interface: 3,
        kind: DeviceKind::Keyboard,
        max_dpi: 0,
        max_stages: 0,
        buttons: 0,
    },
];

pub const DEFAULT_PROFILE_RAW: [u8; 129] = [
    0x00, 0x00, 0xe9, 0x85, 0x8d, 0xe7, 0xbd, 0xae, 0xe6, 0x96, 0x87, 0xe4, 0xbb, 0xb6, 0x31, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xb0, 0x00, 0x00, 0x00, 0x00, 0xb1, 0x00, 0x00, 0x00, 0x00, 0xb2, 0x00, 0x00, 0x00, 0x00, 0xb5,
    0x00, 0x00, 0x00, 0x00, 0xb6, 0x00, 0x00, 0x00, 0x00, 0xb3, 0x00, 0x00, 0x00, 0x00, 0xb4, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0xc1, 0x00, 0x00,
    0x00, 0x00, 0x08, 0x00, 0x08, 0x00, 0x10, 0x00, 0x10, 0x00, 0x20, 0x00, 0x20, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0a, 0x10, 0x00, 0xcc, 0x00, 0x01, 0x64, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00,
];

#[derive(Clone, Copy)]
pub struct ActionDef {
    pub key: &'static str,
    pub label: &'static str,
    pub code: u8,
}

pub const BUTTON_ACTIONS: &[ActionDef] = &[
    ActionDef { key: "left", label: "Left", code: 0xB0 },
    ActionDef { key: "right", label: "Right", code: 0xB1 },
    ActionDef { key: "middle", label: "Middle", code: 0xB2 },
    ActionDef { key: "dpi_switch", label: "DPI switch", code: 0xC1 },
    ActionDef { key: "forward", label: "Forward", code: 0xB3 },
    ActionDef { key: "backward", label: "Backward", code: 0xB4 },
    ActionDef { key: "scroll_up", label: "Wheel up", code: 0xB5 },
    ActionDef { key: "scroll_down", label: "Wheel down", code: 0xB6 },
    ActionDef { key: "play_pause", label: "Play/Pause", code: 0x93 },
    ActionDef { key: "prev_track", label: "Previous track", code: 0x95 },
    ActionDef { key: "next_track", label: "Next track", code: 0x96 },
    ActionDef { key: "vol_up", label: "Volume up", code: 0x90 },
    ActionDef { key: "vol_down", label: "Volume down", code: 0x91 },
    ActionDef { key: "mute", label: "Mute", code: 0x92 },
    ActionDef { key: "disable", label: "Disable", code: 0x00 },
    ActionDef { key: "disable1", label: "Disable (Alt)", code: 0x01 },
];

// UI button indicator 1..8 -> hardware KeyMatrix offset (from M300LayoutKeyboard..cctor)
pub const UI_BTN_TO_HW_OFFSET: [(usize, &'static str); 8] = [
    (0, "1  Left Button"),
    (1, "2  Right Button"),
    (2, "3  Middle Button"),
    (9, "4  DPI Switch"),
    (5, "5  Forward (Side)"),
    (6, "6  Backward (Side)"),
    (8, "7  Right Side Upper"),
    (7, "8  Right Side Lower"),
];

#[derive(Clone, Debug)]
pub struct ParsedProfile {
    pub name: String,
    pub report_rate_hz: u32,
    pub active_dpi_index: usize,
    pub dpi_levels: usize,
    pub dpi_stages: [u32; 5],
    pub lighting_effect: String,
    pub color1: [u8; 3],
    pub color2: [u8; 3],
    pub button_codes: [u8; 10],
}

#[derive(Clone, Copy)]
pub struct KbEffectDef {
    pub key: &'static str,
    pub label: &'static str,
    pub id: u8,
    pub has_aux_color: bool,
}

pub const KB_EFFECTS: &[KbEffectDef] = &[
    KbEffectDef { key: "surfing", label: "Surfing / Wave", id: 11, has_aux_color: false },
    KbEffectDef { key: "static", label: "Static", id: 1, has_aux_color: false },
    KbEffectDef { key: "spiral", label: "Spiral", id: 2, has_aux_color: false },
    KbEffectDef { key: "laser", label: "Laser", id: 3, has_aux_color: false },
    KbEffectDef { key: "drifting", label: "Drifting", id: 4, has_aux_color: false },
    KbEffectDef { key: "fading_trace", label: "Fading Trace", id: 5, has_aux_color: false },
    KbEffectDef { key: "customize", label: "Customize", id: 6, has_aux_color: false },
    KbEffectDef { key: "breathing", label: "Breathing", id: 7, has_aux_color: true },
    KbEffectDef { key: "breathing_rainbow", label: "Breathing (Rainbow)", id: 8, has_aux_color: false },
    KbEffectDef { key: "starlight", label: "Starlight", id: 9, has_aux_color: false },
    KbEffectDef { key: "starlight_dual_colors", label: "Starlight (Dual Colors)", id: 10, has_aux_color: true },
];

#[derive(Clone, Debug)]
pub struct ParsedKbProfile {
    pub effect_id: u8,
    pub brightness: u8,
    pub speed: u8,
    pub color1: [u8; 3],
    pub color2: [u8; 3],
    pub game_mode: bool,
    pub win_lock: bool,
    pub raw: Vec<u8>,
}

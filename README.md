# Legion Accessory Central (Linux)

Native Linux driver and control utility for Lenovo Legion gaming accessories, reverse-engineered from **Legion Accessory Central v2.0.9.10231**.

Built in Rust using `eframe` / `egui`, featuring the original visual identity, official graphics, full dark theme (`#11192C` / `#1F2A42` / `#257EFF`), CLI mode, and zero external daemon dependencies.

---

## Supported Devices

| Model | Type | USB VID:PID | HID Interface | Features |
|---|---|---|---|---|
| **Legion M300** | Mouse | `17ef:60e4` | 2 | DPI, Polling Rate, RGB, 8-Button Remap, Profiles |
| **Legion M300s** | Mouse | `17ef:6132`, `17ef:6155` | 2 | DPI (8k), Polling Rate, RGB, 6-Button Remap |
| **Legion M500** | Mouse | `17ef:60d4` | 2 | DPI (16k), Polling Rate, RGB, 7-Button Remap |
| **Legion M600** | Wireless Mouse | `17ef:60e5`, `17ef:60e6` | 2 | Wired/2.4G, DPI, Polling Rate, RGB, Battery |
| **Legion M600s** | Wireless Mouse | `17ef:6133`, `17ef:6135`, `17ef:6140` | 2 | Wired/2.4G, DPI (19k), Polling Rate, RGB |
| **Legion M600s Qi** | Wireless Mouse | `17ef:6136`, `17ef:6138`, `17ef:6141` | 2 | Wired/2.4G, Qi, DPI (19k), Polling Rate, RGB |
| **Legion K300** | Keyboard | `17ef:60f0` | 3 | Lighting Effects, Speed, Game Mode |
| **Legion K310** | Keyboard | `17ef:6199` | 3 | Lighting Effects, Speed, Game Mode |
| **Legion K310 AI** | Keyboard | `17ef:6205` | 3 | Lighting Effects, Speed, Game Mode |
| **Legion K500** | Mechanical Keyboard | `17ef:60d5` | 3 | RGB Zones, Lighting Effects, Game Mode |

---

## Quick Start

### 1. Install udev permissions rule

Linux defaults `/dev/hidraw*` to `root:root (0600)`. Install the included rule so non-root users can access device endpoints:

```bash
sudo cp 99-lenovo-legion-hid.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

*(Note: The GUI also has a built-in **Unlock USB (Polkit)** button that prompts for privilege escalation once and writes this rule automatically).*

### 2. Build and Run

Requirements: Rust toolchain (`cargo`, `rustc`).

```bash
# Run graphical interface (default)
cargo run --release

# Run protocol self-test
cargo run --release -- --self-test
```

---

## CLI Usage

All device operations can be scripted via command-line arguments:

```bash
# Read mouse configuration & current profile
cargo run --release -- --status

# Adjust DPI stages and active level
cargo run --release -- --dpi 0 800 --dpi 1 1600 --dpi 2 3200
cargo run --release -- --dpi-active 1
cargo run --release -- --dpi-levels 3

# Change polling rate (125, 250, 500, 1000 Hz)
cargo run --release -- --rate 1000

# Set RGB lighting
cargo run --release -- --lighting rainbow
cargo run --release -- --lighting static 257eff
cargo run --release -- --lighting breathing 00ff00 ff0000
cargo run --release -- --lighting off

# Remap buttons (slots 0..9)
cargo run --release -- --button 5 forward
cargo run --release -- --button 6 backward

# Factory reset onboard profile
cargo run --release -- --factory-reset
```

---

## Project Structure

```
.
├── Cargo.toml                  # Rust manifest (eframe, egui, image, libc)
├── 99-lenovo-legion-hid.rules  # Udev rules for all 10 Legion hardware IDs
├── LICENSE                     # MIT License
├── README.md                   # Documentation
├── assets/                     # Embedded official PNG graphics & icons
│   ├── legion_logo.png
│   ├── m300_lighting.png
│   ├── m300_action.png
│   ├── mouse_icon.png
│   └── kb_icon.png
├── src/
│   ├── main.rs                 # Entry point & CLI argument parser
│   ├── gui.rs                  # egui Windows-replica control panel
│   ├── hid.rs                  # Linux /dev/hidraw communication & Polkit helper
│   ├── models.rs               # Hardware PID tables & data structures
│   └── protocol.rs             # 64-byte HID packet builders, parsers & self-tests
└── legion_m300.py              # Single-file standalone Python CLI/GUI script
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
All product and brand names are trademarks of their respective owners.

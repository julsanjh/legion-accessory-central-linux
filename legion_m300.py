#!/usr/bin/env python3
"""
Lenovo Legion Accessory Central (v2.0.9.10231) — Native Linux Driver & Control Utility
Reverse-engineered from setup_v2_0_9_10231.exe (ldm_m300.dll + legion_hid.exe).

Target Hardware:
  Lenovo Legion M300 RGB Gaming Mouse (USB VID 0x17ef, PID 0x60e4, Interface 2)
"""

import argparse
import glob
import os
import select
import struct
import sys
import time

VID_LENOVO = 0x17EF
PID_M300 = 0x60E4
INTERFACE_M300 = 2

# 129-byte factory default profile extracted from Lenny.Module.M300.Protocol.DefaultProfile.Raw
DEFAULT_PROFILE_RAW = bytes.fromhex(
    "0000e9858de7bdaee69687e4bbb6310000000000000000000000000000000000"
    "b000000000b100000000b200000000b500000000b600000000"
    "b300000000b40000000000000000000100000000c100000000"
    "0800080010001000200020000000000000000000"
    "000101020000000000000a1000cc00"
    "016400010000000000000000"
)

REPORT_RATE_TO_CODE = {125: 0x08, 250: 0x04, 500: 0x02, 1000: 0x00}
CODE_TO_REPORT_RATE = {v: k for k, v in REPORT_RATE_TO_CODE.items()}

BUTTON_SLOTS = {
    0: "Left Click (Button 1)",
    1: "Right Click (Button 2)",
    2: "Middle / Wheel Click (Button 3)",
    3: "Scroll Up",
    4: "Scroll Down",
    5: "Forward (Side Button 5)",
    6: "Backward (Side Button 4)",
    7: "Side Button 7",
    8: "Side Button 6",
    9: "DPI Cycle Button",
}

BUTTON_ACTIONS = {
    "left":        bytes([0xB0, 0, 0, 0, 0]),
    "right":       bytes([0xB1, 0, 0, 0, 0]),
    "middle":      bytes([0xB2, 0, 0, 0, 0]),
    "forward":     bytes([0xB3, 0, 0, 0, 0]),
    "backward":    bytes([0xB4, 0, 0, 0, 0]),
    "scroll_up":   bytes([0xB5, 0, 0, 0, 0]),
    "scroll_down": bytes([0xB6, 0, 0, 0, 0]),
    "dpi_switch":  bytes([0xC1, 0, 0, 0, 0]),
    "play_pause":  bytes([0x93, 0, 0, 0, 0]),
    "prev_track":  bytes([0x95, 0, 0, 0, 0]),
    "next_track":  bytes([0x96, 0, 0, 0, 0]),
    "vol_up":      bytes([0x90, 0, 0, 0, 0]),
    "vol_down":    bytes([0x91, 0, 0, 0, 0]),
    "mute":        bytes([0x92, 0, 0, 0, 0]),
    "disable":     bytes([0x00, 0, 0, 0, 0]),
    "disable1":    bytes([0x01, 0, 0, 0, 0]),
}
CODE_TO_ACTION = {v[0]: k for k, v in BUTTON_ACTIONS.items()}


def checksum64(payload: bytes) -> bytes:
    """Pad/truncate to 64 bytes and set byte 63 to sum(b[0..62]) & 0xff (M300Device::WritePacket)."""
    b = bytearray(64)
    n = min(len(payload), 63)
    b[:n] = payload[:n]
    b[63] = sum(b[:63]) & 0xFF
    return bytes(b)


def build_chunked_packets(cmd: int, sub: int, data: bytes) -> list[bytes]:
    """Build 64-byte chunked packets (max 58 bytes/chunk) matching M300Device::SendData (0x180023360)."""
    max_chunk = 0x3A  # 58 bytes
    total_chunks = max(1, (len(data) + max_chunk - 1) // max_chunk)
    packets = []
    for idx in range(total_chunks):
        chunk = data[idx * max_chunk : (idx + 1) * max_chunk]
        pkt = bytearray(64)
        pkt[0] = cmd & 0xFF
        pkt[1] = sub & 0xFF
        pkt[2] = idx + 1
        pkt[3] = total_chunks
        pkt[4] = len(chunk)
        pkt[5 : 5 + len(chunk)] = chunk
        packets.append(checksum64(pkt))
    return packets


def build_lighting_data(effect: str, color1: str = "ff0000", color2: str | None = None) -> bytes:
    """Build 12-byte lighting parameter block matching Lenny.Module.M300.Protocol.Lighting."""
    def parse_rgb(hex_str: str) -> tuple[int, int, int]:
        h = hex_str.lstrip("#")
        if len(h) != 6:
            raise ValueError(f"Expected 6-digit hex color RRGGBB, got {hex_str!r}")
        return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)

    buf = bytearray(12)
    effect = effect.lower()
    if effect == "off":
        buf[0] = 0x03
    elif effect == "rainbow":
        buf[0:4] = bytes([0x01, 0x64, 0x00, 0x0A])
    elif effect == "random":
        buf[0:3] = bytes([0x03, 0x64, 0x01])
    elif effect == "static":
        r, g, b = parse_rgb(color1)
        buf[0:6] = bytes([0x03, 0x64, 0x00, r, g, b])
    elif effect == "breathing":
        r1, g1, b1 = parse_rgb(color1)
        buf[0:4] = bytes([0x02, 0x64, 0x00, 0x02])
        if color2:
            r2, g2, b2 = parse_rgb(color2)
            buf[4] = 0x03
            buf[6:12] = bytes([r1, g1, b1, r2, g2, b2])
        else:
            buf[4] = 0x01
            buf[6:9] = bytes([r1, g1, b1])
    else:
        raise ValueError(f"Unknown lighting effect: {effect}")
    return bytes(buf)


def build_lighting_packet(effect: str, color1: str = "ff0000", color2: str | None = None) -> bytes:
    data12 = build_lighting_data(effect, color1, color2)
    return build_chunked_packets(0x25, 0x00, data12)[0]


def build_dpi_value_packet(index: int, dpi: int, dpi_y: int | None = None) -> bytes:
    """Build packet for /m300/set_dpi_value (0x180003900): [04, 01, 00, index, x_lo, x_hi, y_lo, y_hi]."""
    if not (0 <= index <= 4):
        raise ValueError("DPI stage index must be 0..4")
    if not (200 <= dpi <= 8000) or dpi % 100 != 0:
        raise ValueError("DPI must be between 200 and 8000 in steps of 100")
    dy = dpi if dpi_y is None else dpi_y
    vx = dpi // 100
    vy = dy // 100
    pkt = bytearray(64)
    struct.pack_into("<BBBBHH", pkt, 0, 0x04, 0x01, 0x00, index, vx, vy)
    return bytes(pkt)


def build_dpi_active_packet(index: int) -> bytes:
    """Build packet for /m300/set_dpi_active_index (0x180003710): [04, 01, 01, index]."""
    if not (0 <= index <= 4):
        raise ValueError("Active DPI index must be 0..4")
    pkt = bytearray(64)
    pkt[0:4] = bytes([0x04, 0x01, 0x01, index])
    return bytes(pkt)


def build_dpi_max_level_packet(levels: int) -> bytes:
    """Build packet for /m300/set_dpi_max_level (0x180003520): [04, 01, 02, levels - 1]."""
    if not (1 <= levels <= 5):
        raise ValueError("DPI levels count must be 1..5")
    pkt = bytearray(64)
    pkt[0:4] = bytes([0x04, 0x01, 0x02, levels - 1])
    return bytes(pkt)


def build_report_rate_packet(hz: int) -> bytes:
    """Build packet for /m300/set_report_rate (0x180003c10): [04, 02, 00, rate_code]."""
    if hz not in REPORT_RATE_TO_CODE:
        raise ValueError(f"Polling rate must be one of {sorted(REPORT_RATE_TO_CODE)}")
    pkt = bytearray(64)
    pkt[0:4] = bytes([0x04, 0x02, 0x00, REPORT_RATE_TO_CODE[hz]])
    return bytes(pkt)


def build_key_action_packet(key_index: int, action_name: str) -> bytes:
    """Build packet for /m300/set_keyaction (0x180023802): [02, 00, key_index, a0..a4, ..., checksum]."""
    if not (0 <= key_index <= 9):
        raise ValueError("key_index must be 0..9")
    act = BUTTON_ACTIONS.get(action_name.lower())
    if act is None:
        raise ValueError(f"Unknown action {action_name!r}; valid: {sorted(BUTTON_ACTIONS)}")
    pkt = bytearray(64)
    pkt[0] = 0x02
    pkt[1] = 0x00
    pkt[2] = key_index
    pkt[3:8] = act
    return checksum64(pkt)


def parse_profile(raw: bytes) -> dict:
    """Parse 129-byte M300 profile structure (Lenny.Module.M300.Protocol.Profile.Parse)."""
    if len(raw) < 129:
        raise ValueError(f"Expected 129-byte profile, got {len(raw)} bytes")
    name_bytes = raw[2:32].split(b"\x00", 1)[0]
    name = name_bytes.decode("utf-8", errors="replace")

    buttons = []
    for i in range(10):
        b5 = raw[32 + i * 5 : 32 + (i + 1) * 5]
        action_name = CODE_TO_ACTION.get(b5[0], f"raw:{b5.hex()}")
        if b5[0] == 0xF2:
            action_name = f"macro:{b5[1]}"
        buttons.append({"index": i, "slot": BUTTON_SLOTS[i], "action": action_name, "raw": b5.hex()})

    dpi_stages = []
    for i in range(5):
        vx, vy = struct.unpack_from("<HH", raw, 82 + i * 4)
        dpi_stages.append({"index": i, "dpi_x": vx * 100, "dpi_y": vy * 100})

    rate_code = raw[102]
    current_dpi = raw[103]
    default_dpi = raw[104]
    dpi_count = raw[105] + 1
    surface_thresh = raw[112]
    surface_min_sq = raw[113]
    light = raw[117:129]

    if light[0] == 1:
        effect = "rainbow"
    elif light[0] == 2:
        effect = "breathing_2color" if light[4] == 3 else "breathing"
    elif light[0] == 3:
        if light[1] == 0:
            effect = "off"
        elif light[2] != 0:
            effect = "random"
        else:
            effect = "static"
    else:
        effect = f"unknown({light[0]})"

    color1 = f"#{light[3]:02x}{light[4]:02x}{light[5]:02x}" if effect == "static" else f"#{light[6]:02x}{light[7]:02x}{light[8]:02x}"
    color2 = f"#{light[9]:02x}{light[10]:02x}{light[11]:02x}"

    return {
        "name": name,
        "report_rate_hz": CODE_TO_REPORT_RATE.get(rate_code, rate_code),
        "active_dpi_index": current_dpi,
        "default_dpi_index": default_dpi,
        "dpi_levels": dpi_count,
        "dpi_stages": dpi_stages,
        "surface": (surface_thresh << 8) | surface_min_sq,
        "lighting": {
            "effect": effect,
            "brightness": light[1],
            "color1": color1,
            "color2": color2,
            "raw": light.hex(),
        },
        "buttons": buttons,
    }


def find_m300_hidraw() -> str | None:
    """Locate /dev/hidrawX for Lenovo Legion M300 (17ef:60e4, input2 / interface 2)."""
    target_id = f"0003:{VID_LENOVO:08X}:{PID_M300:08X}"
    fallback = None
    for sys_path in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
        uevent_path = os.path.join(sys_path, "device/uevent")
        if not os.path.exists(uevent_path):
            continue
        with open(uevent_path, encoding="utf-8", errors="ignore") as f:
            uevent = f.read()
        if f"HID_ID={target_id}" in uevent:
            dev_node = f"/dev/{os.path.basename(sys_path)}"
            if f"/input{INTERFACE_M300}" in uevent:
                return dev_node
            fallback = dev_node
    return fallback


class M300Device:
    def __init__(self, path: str | None = None):
        self.path = path or find_m300_hidraw()
        if not self.path:
            raise RuntimeError("Lenovo Legion M300 (17ef:60e4) not found on USB bus.")
        self.fd: int | None = None

    def _unlock_with_pkexec(self) -> bool:
        import shutil
        import subprocess
        if not shutil.which("pkexec"):
            return False
        rule = (
            'SUBSYSTEM=="hidraw", ATTRS{idVendor}=="17ef", '
            'ATTRS{idProduct}=="60e4", MODE="0666", TAG+="uaccess"\n'
        )
        cmd = (
            f"chmod 0666 {self.path} && "
            f"setfacl -m u:{os.getuid()}:rw {self.path} && "
            f"printf '%s' '{rule}' > /etc/udev/rules.d/99-lenovo-legion-hid.rules && "
            f"udevadm control --reload-rules && udevadm trigger"
        )
        res = subprocess.run(["pkexec", "sh", "-c", cmd])
        return res.returncode == 0

    def __enter__(self):
        try:
            self.fd = os.open(self.path, os.O_RDWR | os.O_NONBLOCK)
        except PermissionError as e:
            if self._unlock_with_pkexec():
                self.fd = os.open(self.path, os.O_RDWR | os.O_NONBLOCK)
            else:
                raise PermissionError(
                    f"Permission denied opening {self.path}.\n"
                    f"Run with sudo or click 'Unlock USB (Polkit)'."
                ) from e
        self.clear_queue()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None

    def clear_queue(self):
        assert self.fd is not None
        while select.select([self.fd], [], [], 0.005)[0]:
            try:
                os.read(self.fd, 64)
            except BlockingIOError:
                break

    def write_raw(self, payload64: bytes, delay: float = 0.1):
        """Send 65-byte HID output report (Report ID 0x00 + 64-byte payload)."""
        assert self.fd is not None
        report = bytearray(65)
        report[0] = 0x00
        report[1 : 1 + min(len(payload64), 64)] = payload64[:64]
        time.sleep(delay)
        os.write(self.fd, bytes(report))
        time.sleep(delay)

    def read_packet(self, timeout: float = 0.5) -> bytes | None:
        assert self.fd is not None
        r, _, _ = select.select([self.fd], [], [], timeout)
        if not r:
            return None
        return os.read(self.fd, 64)

    def sync(self) -> dict:
        """Execute /m300/sync (0x180022e90): send [03, ff] and reassemble 261-byte state."""
        self.clear_queue()
        self.write_raw(bytes([0x03, 0xFF]))
        buf = bytearray()
        while True:
            pkt = self.read_packet(timeout=0.8)
            if not pkt or len(pkt) < 5:
                break
            pkt_idx = pkt[2]
            pkt_total = pkt[3]
            pkt_len = pkt[4]
            buf.extend(pkt[5 : 5 + pkt_len])
            if pkt_idx >= pkt_total and pkt_total > 0:
                break
        if len(buf) < 132:
            raise RuntimeError(f"Incomplete sync response ({len(buf)} bytes): {buf.hex()}")
        if buf[1] != 0:
            self.commit()
        profile = parse_profile(bytes(buf[3:132]))
        profile["current_profile_index"] = buf[1]
        return profile

    def commit(self):
        """Send commit packet [05, 00, 00] (0x1800232a0)."""
        self.clear_queue()
        self.write_raw(bytes([0x05, 0x00, 0x00]))

    def set_lighting(self, effect: str, color1: str = "ff0000", color2: str | None = None):
        pkt = build_lighting_packet(effect, color1, color2)
        self.write_raw(pkt, delay=0.05)

    def set_dpi_value(self, index: int, dpi: int):
        self.write_raw(build_dpi_value_packet(index, dpi))
        self.read_packet(timeout=0.3)

    def set_dpi_active(self, index: int):
        self.write_raw(build_dpi_active_packet(index))
        self.read_packet(timeout=0.3)

    def set_dpi_levels(self, levels: int):
        self.write_raw(build_dpi_max_level_packet(levels))
        self.read_packet(timeout=0.3)

    def set_report_rate(self, hz: int):
        self.write_raw(build_report_rate_packet(hz))

    def set_key_action(self, key_index: int, action_name: str):
        self.write_raw(build_key_action_packet(key_index, action_name))
        self.read_packet(timeout=0.3)

    def send_profile(self, profile_raw: bytes = DEFAULT_PROFILE_RAW):
        for pkt in build_chunked_packets(0x01, 0x00, profile_raw):
            self.write_raw(pkt, delay=0.05)
        self.commit()

    def factory_reset(self):
        """Execute /m300/restore_factory (0x180004060)."""
        self.write_raw(bytes([0x07, 0x05, 0x00]))
        time.sleep(0.1)
        self.write_raw(bytes([0x07, 0x05, 0x01]))
        time.sleep(0.1)
        self.commit()


def run_self_test():
    """Verify protocol builders & parsers against exact binary vectors from legion_hid.exe & ldm_m300.dll."""
    assert len(DEFAULT_PROFILE_RAW) == 129
    prof = parse_profile(DEFAULT_PROFILE_RAW)
    assert prof["name"] == "配置文件1"
    assert prof["report_rate_hz"] == 1000
    assert prof["active_dpi_index"] == 1
    assert prof["dpi_levels"] == 3
    assert [s["dpi_x"] for s in prof["dpi_stages"][:3]] == [800, 1600, 3200]
    assert prof["buttons"][0]["action"] == "left"
    assert prof["buttons"][1]["action"] == "right"
    assert prof["buttons"][2]["action"] == "middle"
    assert prof["buttons"][9]["action"] == "dpi_switch"

    # Verify static arrays from legion_hid.exe FieldRVA
    assert build_lighting_data("rainbow") == bytes.fromhex("0164000a0000000000000000")
    assert build_lighting_data("random") == bytes.fromhex("036401000000000000000000")
    assert build_lighting_data("off") == bytes.fromhex("030000000000000000000000")
    assert build_lighting_data("static", "257eff") == bytes.fromhex("036400257eff000000000000")
    assert build_lighting_data("breathing", "ff0000", "0000ff") == bytes.fromhex("026400020300ff00000000ff")

    lpkt = build_lighting_packet("static", "ff0000")
    assert len(lpkt) == 64
    assert lpkt[:11] == bytes([0x25, 0x00, 0x01, 0x01, 0x0C, 0x03, 0x64, 0x00, 0xFF, 0x00, 0x00])
    assert lpkt[63] == sum(lpkt[:63]) & 0xFF

    dpkt = build_dpi_value_packet(1, 1600)
    assert dpkt[:8] == bytes([0x04, 0x01, 0x00, 0x01, 16, 0, 16, 0])
    assert build_dpi_active_packet(2)[:4] == bytes([0x04, 0x01, 0x01, 0x02])
    assert build_dpi_max_level_packet(3)[:4] == bytes([0x04, 0x01, 0x02, 0x02])
    assert build_report_rate_packet(500)[:4] == bytes([0x04, 0x02, 0x00, 0x02])

    kpkt = build_key_action_packet(5, "forward")
    assert kpkt[:8] == bytes([0x02, 0x00, 0x05, 0xB3, 0x00, 0x00, 0x00, 0x00])
    assert kpkt[63] == (0x02 + 0x05 + 0xB3) & 0xFF

    chunks = build_chunked_packets(0x01, 0x00, DEFAULT_PROFILE_RAW)
    assert len(chunks) == 3  # 58 + 58 + 13 = 129
    assert [c[4] for c in chunks] == [58, 58, 13]
    print("Self-test passed: all M300 protocol vectors verified.")


def launch_gui(device_path: str | None = None):
    if not os.environ.get("DISPLAY"):
        os.environ["DISPLAY"] = ":0"
    if not os.environ.get("XAUTHORITY"):
        uid = os.environ.get("SUDO_UID") or str(os.getuid())
        xauths = sorted(glob.glob(f"/run/user/{uid}/.mutter-Xwaylandauth.*"))
        if xauths:
            os.environ["XAUTHORITY"] = xauths[-1]
    import tkinter as tk
    from tkinter import colorchooser, messagebox, ttk

    root = tk.Tk()
    root.title("Lenovo Legion M300 — Linux Control Center")
    root.geometry("680x560")
    root.configure(bg="#141a29")

    style = ttk.Style(root)
    style.theme_use("clam")
    style.configure(".", background="#141a29", foreground="#e6edf3", fieldbackground="#1f2a42")
    style.configure("TLabel", background="#141a29", foreground="#e6edf3", font=("Sans", 10))
    style.configure("Header.TLabel", font=("Sans", 12, "bold"), foreground="#257eff")
    style.configure("TButton", background="#257eff", foreground="#ffffff", padding=6)
    style.map("TButton", background=[("active", "#1b5fcc")])

    detected = device_path or find_m300_hidraw() or "Not found"
    status_var = tk.StringVar(value=f"Device: {detected} (VID:17ef PID:60e4)")

    hdr = ttk.Frame(root, padding=12)
    hdr.pack(fill="x")
    ttk.Label(hdr, text="Lenovo Legion M300 RGB Gaming Mouse", style="Header.TLabel").pack(side="left")
    ttk.Label(hdr, textvariable=status_var).pack(side="right")

    nb = ttk.Notebook(root)
    nb.pack(fill="both", expand=True, padx=12, pady=6)

    # --- Tab 1: Performance (DPI & Polling Rate) ---
    tab_perf = ttk.Frame(nb, padding=16)
    nb.add(tab_perf, text="Performance (DPI & Rate)")

    dpi_vars = [tk.IntVar(value=v) for v in (800, 1600, 3200)]
    active_dpi_var = tk.IntVar(value=1)
    levels_var = tk.IntVar(value=3)
    rate_var = tk.IntVar(value=1000)

    lvl_frame = ttk.Frame(tab_perf)
    lvl_frame.pack(fill="x", pady=(0, 12))
    ttk.Label(lvl_frame, text="Number of DPI Stages (1-3):").pack(side="left")
    ttk.Spinbox(lvl_frame, from_=1, to=3, textvariable=levels_var, width=5).pack(side="left", padx=8)

    def apply_dpi_stage(idx: int):
        val = max(200, min(8000, round(dpi_vars[idx].get() / 100) * 100))
        dpi_vars[idx].set(val)
        try:
            with M300Device(device_path) as dev:
                dev.set_dpi_value(idx, val)
            status_var.set(f"Set DPI Stage {idx + 1} = {val} DPI")
        except Exception as e:
            messagebox.showerror("HID Error", str(e))

    for i in range(3):
        row = ttk.Frame(tab_perf)
        row.pack(fill="x", pady=6)
        ttk.Radiobutton(row, text=f"Stage {i + 1}", variable=active_dpi_var, value=i).pack(side="left")
        scale = tk.Scale(
            row, from_=200, to=8000, resolution=100, orient="horizontal",
            variable=dpi_vars[i], bg="#141a29", fg="#e6edf3", highlightthickness=0, length=320
        )
        scale.pack(side="left", padx=12)
        ttk.Button(row, text="Apply DPI", command=lambda idx=i: apply_dpi_stage(idx)).pack(side="left")

    def apply_active_and_levels():
        try:
            with M300Device(device_path) as dev:
                dev.set_dpi_levels(levels_var.get())
                dev.set_dpi_active(active_dpi_var.get())
            status_var.set(f"Active Stage={active_dpi_var.get() + 1}, Stages={levels_var.get()}")
        except Exception as e:
            messagebox.showerror("HID Error", str(e))

    ttk.Button(tab_perf, text="Apply Active Stage & Stage Count", command=apply_active_and_levels).pack(anchor="w", pady=8)

    rate_frame = ttk.Frame(tab_perf)
    rate_frame.pack(fill="x", pady=16)
    ttk.Label(rate_frame, text="Polling Rate (Hz):", style="Header.TLabel").pack(anchor="w", pady=(0, 8))
    for hz in (125, 250, 500, 1000):
        ttk.Radiobutton(rate_frame, text=f"{hz} Hz", variable=rate_var, value=hz).pack(side="left", padx=10)

    def apply_rate():
        try:
            with M300Device(device_path) as dev:
                dev.set_report_rate(rate_var.get())
            status_var.set(f"Set Polling Rate = {rate_var.get()} Hz")
        except Exception as e:
            messagebox.showerror("HID Error", str(e))

    ttk.Button(tab_perf, text="Apply Polling Rate", command=apply_rate).pack(anchor="w")

    # --- Tab 2: RGB Lighting ---
    tab_light = ttk.Frame(nb, padding=16)
    nb.add(tab_light, text="RGB Lighting")

    effect_var = tk.StringVar(value="rainbow")
    color1_var = tk.StringVar(value="257eff")
    color2_var = tk.StringVar(value="ff0055")

    ttk.Label(tab_light, text="Lighting Effect:", style="Header.TLabel").pack(anchor="w", pady=(0, 8))
    for eff in ("rainbow", "static", "breathing", "random", "off"):
        ttk.Radiobutton(tab_light, text=eff.capitalize(), variable=effect_var, value=eff).pack(anchor="w", pady=2)

    c_frame = ttk.Frame(tab_light)
    c_frame.pack(fill="x", pady=12)
    ttk.Label(c_frame, text="Primary Hex (RRGGBB):").grid(row=0, column=0, sticky="w", pady=4)
    ttk.Entry(c_frame, textvariable=color1_var, width=10).grid(row=0, column=1, padx=8)

    def pick_color(var: tk.StringVar):
        _, hx = colorchooser.askcolor(title="Select LED Color")
        if hx:
            var.set(hx.lstrip("#"))

    ttk.Button(c_frame, text="Pick...", command=lambda: pick_color(color1_var)).grid(row=0, column=2)
    ttk.Label(c_frame, text="Secondary Hex (Breathing 2-color, optional):").grid(row=1, column=0, sticky="w", pady=4)
    ttk.Entry(c_frame, textvariable=color2_var, width=10).grid(row=1, column=1, padx=8)
    ttk.Button(c_frame, text="Pick...", command=lambda: pick_color(color2_var)).grid(row=1, column=2)

    def apply_lighting():
        try:
            c2 = color2_var.get().strip() or None
            with M300Device(device_path) as dev:
                dev.set_lighting(effect_var.get(), color1_var.get().strip(), c2)
            status_var.set(f"Lighting updated: {effect_var.get()}")
        except Exception as e:
            messagebox.showerror("HID Error", str(e))

    ttk.Button(tab_light, text="Apply Lighting", command=apply_lighting).pack(anchor="w", pady=8)

    # --- Tab 3: Buttons ---
    tab_btn = ttk.Frame(nb, padding=16)
    nb.add(tab_btn, text="Buttons")

    btn_index_var = tk.IntVar(value=5)
    btn_action_var = tk.StringVar(value="forward")

    ttk.Label(tab_btn, text="Button Slot:", style="Header.TLabel").pack(anchor="w", pady=(0, 6))
    slot_combo = ttk.Combobox(
        tab_btn, state="readonly", width=38,
        values=[f"{k}: {v}" for k, v in BUTTON_SLOTS.items()]
    )
    slot_combo.current(5)
    slot_combo.pack(anchor="w", pady=4)

    ttk.Label(tab_btn, text="Assigned Action:", style="Header.TLabel").pack(anchor="w", pady=(12, 6))
    act_combo = ttk.Combobox(tab_btn, state="readonly", width=24, values=list(BUTTON_ACTIONS.keys()))
    act_combo.set("forward")
    act_combo.pack(anchor="w", pady=4)

    def apply_button():
        idx = int(slot_combo.get().split(":", 1)[0])
        act = act_combo.get()
        try:
            with M300Device(device_path) as dev:
                dev.set_key_action(idx, act)
            status_var.set(f"Button {idx} -> {act}")
        except Exception as e:
            messagebox.showerror("HID Error", str(e))

    ttk.Button(tab_btn, text="Apply Button Mapping", command=apply_button).pack(anchor="w", pady=12)

    # --- Bottom bar: Sync from Mouse / Factory Reset ---
    bot = ttk.Frame(root, padding=12)
    bot.pack(fill="x")

    def sync_from_mouse():
        try:
            with M300Device(device_path) as dev:
                prof = dev.sync()
            for i in range(3):
                dpi_vars[i].set(prof["dpi_stages"][i]["dpi_x"])
            active_dpi_var.set(prof["active_dpi_index"])
            levels_var.set(min(3, prof["dpi_levels"]))
            if isinstance(prof["report_rate_hz"], int):
                rate_var.set(prof["report_rate_hz"])
            status_var.set(f"Synced: {prof['name']} | {prof['report_rate_hz']}Hz | {prof['lighting']['effect']}")
        except Exception as e:
            messagebox.showerror("Sync Error", str(e))

    def reset_mouse():
        if not messagebox.askyesno("Factory Reset", "Restore Lenovo Legion M300 to factory defaults?"):
            return
        try:
            with M300Device(device_path) as dev:
                dev.factory_reset()
                dev.send_profile(DEFAULT_PROFILE_RAW)
            status_var.set("Restored factory defaults.")
        except Exception as e:
            messagebox.showerror("Reset Error", str(e))

    def unlock_usb():
        try:
            dev = M300Device(device_path)
            if dev._unlock_with_pkexec():
                status_var.set(f"Unlocked USB access on {dev.path}")
                sync_from_mouse()
        except Exception as e:
            messagebox.showerror("USB Error", str(e))
    ttk.Button(bot, text="Read / Sync from Mouse", command=sync_from_mouse).pack(side="left")
    ttk.Button(bot, text="Unlock USB (Polkit)", command=unlock_usb).pack(side="left", padx=8)
    ttk.Button(bot, text="Restore Factory Defaults", command=reset_mouse).pack(side="right")

    root.mainloop()


def main():
    parser = argparse.ArgumentParser(
        description="Lenovo Legion M300 RGB Gaming Mouse — Native Linux Control Tool"
    )
    parser.add_argument("-d", "--device", help="HID device path (auto-detected if omitted)")
    parser.add_argument("--gui", action="store_true", help="Launch graphical interface")
    parser.add_argument("--self-test", action="store_true", help="Run protocol self-tests")
    parser.add_argument("--status", action="store_true", help="Read and print current mouse profile/settings")
    parser.add_argument(
        "--lighting", nargs="+", metavar=("EFFECT", "COLOR"),
        help="Set lighting: off | rainbow | random | static RRGGBB | breathing RRGGBB [RRGGBB2]"
    )
    parser.add_argument(
        "--dpi", nargs=2, type=int, metavar=("STAGE_IDX", "DPI"),
        help="Set DPI for stage 0..4 (200..8000, step 100)"
    )
    parser.add_argument("--dpi-active", type=int, metavar="STAGE_IDX", help="Set active DPI stage (0..4)")
    parser.add_argument("--dpi-levels", type=int, metavar="COUNT", help="Set number of DPI stages (1..5)")
    parser.add_argument("--rate", type=int, choices=[125, 250, 500, 1000], help="Set polling rate in Hz")
    parser.add_argument(
        "--button", nargs=2, metavar=("SLOT_IDX", "ACTION"),
        help=f"Map button 0..9 to action ({', '.join(BUTTON_ACTIONS)})"
    )
    parser.add_argument("--factory-reset", action="store_true", help="Restore mouse to factory defaults")

    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return

    any_cli = any([
        args.status, args.lighting, args.dpi,
        args.dpi_active is not None, args.dpi_levels is not None,
        args.rate is not None, args.button, args.factory_reset
    ])

    if args.gui or (not any_cli and os.environ.get("DISPLAY")):
        launch_gui(args.device)
        return

    if not any_cli:
        parser.print_help()
        return

    with M300Device(args.device) as dev:
        if args.factory_reset:
            dev.factory_reset()
            dev.send_profile(DEFAULT_PROFILE_RAW)
            print("Factory defaults restored.")
        if args.lighting:
            eff = args.lighting[0]
            c1 = args.lighting[1] if len(args.lighting) > 1 else "ff0000"
            c2 = args.lighting[2] if len(args.lighting) > 2 else None
            dev.set_lighting(eff, c1, c2)
            print(f"Lighting set to {eff}.")
        if args.dpi:
            idx, val = args.dpi
            dev.set_dpi_value(idx, val)
            print(f"DPI stage {idx} set to {val} DPI.")
        if args.dpi_levels is not None:
            dev.set_dpi_levels(args.dpi_levels)
            print(f"DPI levels count set to {args.dpi_levels}.")
        if args.dpi_active is not None:
            dev.set_dpi_active(args.dpi_active)
            print(f"Active DPI stage set to {args.dpi_active}.")
        if args.rate is not None:
            dev.set_report_rate(args.rate)
            print(f"Polling rate set to {args.rate} Hz.")
        if args.button:
            slot_idx = int(args.button[0])
            act = args.button[1]
            dev.set_key_action(slot_idx, act)
            print(f"Button {slot_idx} ({BUTTON_SLOTS.get(slot_idx, '?')}) mapped to {act}.")
        if args.status:
            prof = dev.sync()
            print(json.dumps(prof, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    import json
    main()

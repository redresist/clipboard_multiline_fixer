#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated, non_snake_case, dead_code)]

mod fixer;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use winit::event_loop::{ControlFlow, EventLoop};

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
extern "system" {
    fn AllocConsole() -> i32;
    fn FreeConsole() -> i32;
    fn SendInput(cInputs: u32, pInputs: *mut INPUT, cbSize: i32) -> u32;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
const INPUT_KEYBOARD: u32 = 1;
#[cfg(windows)]
const KEYEVENTF_KEYUP: u32 = 2;
#[cfg(windows)]
const VK_LSHIFT: u16 = 0xA0;
const VK_INSERT: u16 = 0x2D;
const VK_LCONTROL: u16 = 0xA2;
const VK_V: u16 = 0x56;

#[cfg(windows)]
#[repr(C)]
struct KEYBDINPUT {
    wVk: u16,
    wScan: u16,
    dwFlags: u32,
    time: u32,
    dwExtraInfo: usize,
}

#[cfg(windows)]
#[repr(C)]
struct INPUT {
    type_: u32,
    ki: KEYBDINPUT,
}

static CONSOLE_ENABLED: AtomicBool = AtomicBool::new(false);
static HOTKEY_BUSY: AtomicBool = AtomicBool::new(false);

fn debug_log(msg: &str) {
    if CONSOLE_ENABLED.load(Ordering::Relaxed) {
        eprintln!("[cfx] {}", msg);
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let paused = Arc::new(AtomicBool::new(false));

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Clipboard error: {}", e);
            return;
        }
    };

    let _hotkey_manager = GlobalHotKeyManager::new().expect("Failed to create hotkey manager");
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Insert);
    if let Err(e) = _hotkey_manager.register(hotkey) {
        debug_log(&format!("Hotkey registration failed: {}", e));
    } else {
        debug_log("Hotkey Ctrl+Shift+Ins registered");
    }

    let tray_menu = Menu::new();
    let pause_item = MenuItem::new("Pause", true, None);
    let autostart_item = CheckMenuItem::new("Start with Windows", true, false, None);
    let console_item = CheckMenuItem::new("Show Console", true, false, None);
    let quit_item = MenuItem::new("Quit", true, None);

    tray_menu.append(&pause_item).ok();
    tray_menu.append(&PredefinedMenuItem::separator()).ok();
    tray_menu.append(&autostart_item).ok();
    tray_menu.append(&console_item).ok();
    tray_menu.append(&PredefinedMenuItem::separator()).ok();
    tray_menu.append(&quit_item).ok();

    let active_icon_data = create_icon_data(true);
    let _tray = match Icon::from_rgba(active_icon_data, 32, 32) {
        Ok(icon) => TrayIconBuilder::new()
            .with_tooltip("Clipboard Fixer")
            .with_icon(icon)
            .with_menu(Box::new(tray_menu))
            .build()
            .ok(),
        Err(_) => None,
    };

    let autostart = get_autostart();
    autostart_item.set_checked(autostart);

    let mut last_content = String::new();
    let mut last_check = Instant::now();
    let check_interval = Duration::from_millis(300);

    let menu_rx = MenuEvent::receiver();
    let tray_rx = TrayIconEvent::receiver();
    let hotkey_rx = GlobalHotKeyEvent::receiver();

    let _ = event_loop.run(move |event, target| {
        let _ = &_hotkey_manager;
        if let winit::event::Event::AboutToWait = event {
            while let Ok(ev) = menu_rx.try_recv() {
                if ev.id == pause_item.id() {
                    let was_paused = paused.fetch_xor(true, Ordering::Relaxed);
                    pause_item.set_text(if was_paused { "Pause" } else { "Resume" });
                    debug_log(if was_paused { "Paused" } else { "Resumed" });
                } else if ev.id == autostart_item.id() {
                    let new = !is_autostart_enabled();
                    set_autostart(new);
                    autostart_item.set_checked(new);
                    debug_log(&format!("Auto-start: {}", if new { "ON" } else { "OFF" }));
                } else if ev.id == console_item.id() {
                    toggle_console();
                    console_item.set_checked(CONSOLE_ENABLED.load(Ordering::Relaxed));
                } else if ev.id == quit_item.id() {
                    debug_log("Quitting");
                    target.exit();
                }
            }

            while let Ok(_ev) = tray_rx.try_recv() {}

            while let Ok(_ev) = hotkey_rx.try_recv() {
                if !paused.load(Ordering::Relaxed) && !HOTKEY_BUSY.swap(true, Ordering::AcqRel) {
                    fix_and_paste(&mut clipboard);
                    while let Ok(_) = hotkey_rx.try_recv() {}
                    HOTKEY_BUSY.store(false, Ordering::Release);
                }
            }

            if !paused.load(Ordering::Relaxed) {
                let now = Instant::now();
                if now.duration_since(last_check) >= check_interval {
                    last_check = now;
                    if let Ok(content) = clipboard.get_text() {
                        if content != last_content {
                            debug_log(&format!(
                                "Clipboard changed: {} chars",
                                content.len()
                            ));
                            if CONSOLE_ENABLED.load(Ordering::Relaxed) && content.len() < 200 {
                                debug_log(&format!("Raw: {:?}", content));
                            }
                            let fixed = fixer::fix(&content);
                            if fixed != content {
                                debug_log("Auto-fix: wrapped lines rejoined");
                                if CONSOLE_ENABLED.load(Ordering::Relaxed)
                                    && fixed.len() < 200
                                {
                                    debug_log(&format!("Fixed: {:?}", fixed));
                                }
                                let _ = clipboard.set_text(&fixed);
                            }
                            last_content = fixed;
                        }
                    }
                }
            }

            target.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + check_interval,
            ));
        }
    });
}

fn toggle_console() {
    let was_enabled = CONSOLE_ENABLED.fetch_xor(true, Ordering::Relaxed);
    if !was_enabled {
        #[cfg(windows)]
        unsafe {
            if AllocConsole() != 0 {
                println!("[cfx] Console attached");
                println!("[cfx] Clipboard Multiline Fixer running");
                println!("[cfx] Ctrl+Shift+Ins = fix & paste");
                println!("[cfx] Uncheck 'Show Console' in tray to hide");
            }
        }
    } else {
        #[cfg(windows)]
        unsafe {
            FreeConsole();
        }
    }
}

fn fix_and_paste(clipboard: &mut Clipboard) {
    debug_log("Hotkey: fixing clipboard");

    let content = match clipboard.get_text() {
        Ok(c) => c,
        Err(e) => {
            debug_log(&format!("Clipboard read error: {}", e));
            return;
        }
    };

    debug_log(&format!("Read {} chars from clipboard", content.len()));
    if CONSOLE_ENABLED.load(Ordering::Relaxed) && content.len() < 200 {
        debug_log(&format!("Content: {:?}", content));
    }

    let fixed = fixer::fix(&content);
    if fixed != content {
        debug_log("Wrapped lines detected and fixed");
        if CONSOLE_ENABLED.load(Ordering::Relaxed) && fixed.len() < 200 {
            debug_log(&format!("Fixed: {:?}", fixed));
        }
        if let Err(e) = clipboard.set_text(&fixed) {
            debug_log(&format!("Clipboard write error: {}", e));
            return;
        }
    } else {
        debug_log("No wrapping detected");
    }

    debug_log("Simulating Shift+Insert paste");
    if simulate_shift_insert() {
        debug_log("Shift+Insert sent successfully");
    } else {
        debug_log("Shift+Insert failed, trying Ctrl+V");
        if simulate_ctrl_v() {
            debug_log("Ctrl+V sent successfully");
        } else {
            debug_log("All paste methods failed — clipboard was fixed, paste manually with Shift+Insert");
        }
    }
}

#[cfg(windows)]
fn simulate_shift_insert() -> bool {
    unsafe {
        // User is holding Ctrl+Shift from the hotkey.
        // Release both first, then press Shift+Insert, then release.
        let mut inputs = [
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LCONTROL, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LSHIFT,   wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LSHIFT,   wScan: 0, dwFlags: 0,               time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_INSERT,   wScan: 0, dwFlags: 0,               time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_INSERT,   wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LSHIFT,   wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
        ];
        let result = SendInput(6, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
        if result == 0 {
            debug_log(&format!("SendInput Shift+Insert failed, error: {}", GetLastError()));
        }
        result > 0
    }
}

#[cfg(windows)]
fn simulate_ctrl_v() -> bool {
    unsafe {
        let mut inputs = [
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LSHIFT,   wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LCONTROL, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LCONTROL, wScan: 0, dwFlags: 0,               time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_V,        wScan: 0, dwFlags: 0,               time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_V,        wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            INPUT { type_: INPUT_KEYBOARD, ki: KEYBDINPUT { wVk: VK_LCONTROL, wScan: 0, dwFlags: KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
        ];
        let result = SendInput(6, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
        result > 0
    }
}

#[cfg(not(windows))]
fn simulate_shift_insert() -> bool {
    false
}

#[cfg(not(windows))]
fn simulate_ctrl_v() -> bool {
    false
}

#[cfg(windows)]
fn get_autostart() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
        .ok()
        .and_then(|key| key.get_value::<String, _>("ClipboardFixer").ok())
        .is_some()
}

#[cfg(windows)]
fn set_autostart(enabled: bool) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_WRITE,
    ) {
        if enabled {
            if let Ok(exe) = std::env::current_exe() {
                let _ = run.set_value("ClipboardFixer", &exe.to_string_lossy().to_string());
            }
        } else {
            let _ = run.delete_value("ClipboardFixer");
        }
    }
}

#[cfg(not(windows))]
fn get_autostart() -> bool {
    false
}

#[cfg(not(windows))]
fn set_autostart(_enabled: bool) {}

fn is_autostart_enabled() -> bool {
    get_autostart()
}

fn create_icon_data(active: bool) -> Vec<u8> {
    let size = 32u32;
    let mut data = vec![0u8; (size * size * 4) as usize];

    let (r, g, b) = if active {
        (20, 180, 100)
    } else {
        (140, 140, 140)
    };

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            let outer = x >= 4 && x <= 27 && y >= 6 && y <= 27;
            let inner = x >= 7 && x <= 24 && y >= 9 && y <= 24;
            let top_tab = x >= 9 && x <= 22 && y >= 3 && y <= 7;

            if (outer && !inner) || top_tab {
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = 255;
            }

            if active {
                let cx = x as i32 - 9;
                let cy = y as i32 - 14;

                if cy == cx && cx >= 0 && cx <= 5 {
                    data[idx] = 255;
                    data[idx + 1] = 255;
                    data[idx + 2] = 255;
                    data[idx + 3] = 255;
                }
                if cy == cx + 3 && cx >= -3 && cx <= 1 && cx + 3 >= 0 {
                    data[idx] = 255;
                    data[idx + 1] = 255;
                    data[idx + 2] = 255;
                    data[idx + 3] = 255;
                }
            }
        }
    }

    data
}

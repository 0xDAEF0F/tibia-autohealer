use anyhow::{anyhow, ensure, Context, Result};
use enigo::{Enigo, Key, KeyboardControllable};
use regex::Regex;
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::thread::{self, sleep};
use std::time::Duration;
use xcap::Window;

// RGB colors for health marker
const FULL_GREEN: Rgb = Rgb(0, 137, 0);
const GREENISH: Rgb = Rgb(79, 114, 3);
const YELLOW: Rgb = Rgb(144, 110, 6);
const RED: Rgb = Rgb(137, 34, 34);
const DEEP_RED: Rgb = Rgb(137, 0, 0);
const RED_WINE: Rgb = Rgb(69, 0, 0);

// RGB colors for attack
// const ATTACK_AVAILABLE: RGB = RGB(56, 11, 0);
const ATTACK_IN_COOLDOWN: Rgb = Rgb(184, 38, 1);

#[derive(Copy, Clone, PartialEq)]
struct Rgb(u8, u8, u8);

#[derive(Copy, Clone)]
struct TibiaMarkers {
    health_marker_one: Rgb,
    health_marker_two: Rgb,
    health_marker_three: Rgb,
    attack_marker: Rgb,
}

impl TibiaMarkers {
    fn get_markers(window: &Window) -> Result<TibiaMarkers> {
        let capture = window.capture_image()?;

        // The first pixel of the health bar at the top.
        // It is used to check the color of the bar.
        // x: 12 y: 58 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(24, 66)
            .context("could not get pixels on first health marker")?
            .0;
        let health_marker_one = Rgb(r, g, b);

        // This is still greenish but exura is not enough
        // to heal the character to max level
        // x: 360 y: 58 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(720, 66)
            .context("could not get pixels on second health marker")?
            .0;
        let health_marker_two = Rgb(r, g, b);

        // This marker is to exura when the char
        // receieves more than 50 damage.
        // x: 490 y: 58 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(980, 66)
            .context("could not get pixels on third health marker")?
            .0;
        let health_marker_three = Rgb(r, g, b);

        // Attack marker to know when cooldown is over
        // x: 13 y: 786 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(26, 1522)
            .context("could not get pixels on attack marker")?
            .0;
        let attack_marker = Rgb(r, g, b);

        Ok(TibiaMarkers {
            health_marker_one,
            health_marker_two,
            health_marker_three,
            attack_marker,
        })
    }
}

// based on an image capture determine which key needs pressing for auto healing
// if none then do nothing
fn auto_healing_task(markers: &TibiaMarkers) -> Option<Key> {
    let health_marker_one = markers.health_marker_one;
    let health_marker_two = markers.health_marker_two;
    let health_marker_three = markers.health_marker_three;

    if health_marker_one == DEEP_RED || health_marker_one == RED_WINE || health_marker_one == RED {
        // exura vita
        Some(Key::F2)
    } else if health_marker_one == YELLOW {
        // exura gran
        Some(Key::F3)
    } else if !(health_marker_two == GREENISH || health_marker_two == FULL_GREEN) {
        // exura gran
        Some(Key::F3)
    } else if !(health_marker_three == GREENISH || health_marker_three == FULL_GREEN) {
        // exura
        Some(Key::F4)
    } else {
        None
    }
}

fn attack_cooldown_task(tibia_markers: TibiaMarkers) {
    let attack_marker = tibia_markers.attack_marker;

    if attack_marker == ATTACK_IN_COOLDOWN {
        sleep(Duration::from_millis(1900));
        beep().unwrap();
    } else {
        sleep(Duration::from_millis(50));
    }
}

fn main() -> Result<()> {
    let windows = Window::all()?;
    let tibia_window = windows
        .into_iter()
        .find(|w| w.app_name() == "Tibia")
        .ok_or(anyhow!("tibia not opened"))?;

    ensure!(
        tibia_window.width() == 1440 && tibia_window.height() == 875,
        "mismatch in window size"
    );

    let tibia_markers = Arc::new(RwLock::new(TibiaMarkers::get_markers(&tibia_window)?));

    // healing thread
    let tibia_markers_clone = Arc::clone(&tibia_markers);
    thread::spawn(move || {
        let mut enigo = Enigo::new();
        loop {
            if macos_get_active_window_app_name() != "Tibia" {
                sleep(Duration::from_secs(3));
                continue;
            }

            let tm = *tibia_markers_clone.read().unwrap();

            match auto_healing_task(&tm) {
                Some(key) => {
                    enigo.key_click(key);
                    sleep(Duration::from_secs(1));
                }
                None => sleep(Duration::from_millis(50)),
            }
        }
    });

    // beep thread
    let tibia_markers_clone = Arc::clone(&tibia_markers);
    thread::spawn(move || loop {
        let latest_tibia_markers = *tibia_markers_clone.read().unwrap();
        attack_cooldown_task(latest_tibia_markers);
    });

    // main thread will just update markers every 50ms
    loop {
        let tm = TibiaMarkers::get_markers(&tibia_window)?;
        *tibia_markers.write().unwrap() = tm;
        sleep(Duration::from_millis(50));
    }
}

fn beep() -> Result<()> {
    Command::new("osascript").arg("-e").arg("beep").output()?;
    Ok(())
}

fn macos_get_active_window_app_name() -> String {
    let output = Command::new("sh")
        .arg("-c")
        .arg("lsappinfo | grep 'front'")
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to execute command");

    let mut app_name = String::from_utf8(output.stdout)
        .expect("Output was not UTF-8")
        .replace('\n', "");

    app_name = extract_app_name(app_name);
    app_name
}

fn extract_app_name(app_name: String) -> String {
    let re = Regex::new(r#""([^"]+)""#).unwrap();

    match re.captures(&app_name) {
        Some(caps) => caps
            .get(1)
            .map_or_else(|| app_name.clone(), |m| m.as_str().to_string()),
        None => app_name,
    }
}

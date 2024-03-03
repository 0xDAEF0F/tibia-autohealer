use anyhow::{anyhow, ensure, Context, Ok, Result};
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
const ATTACK_AVAILABLE: Rgb = Rgb(56, 11, 0);
const ATTACK_IN_COOLDOWN: Rgb = Rgb(184, 38, 1);

#[derive(PartialEq, Eq, Copy, Clone)]
struct Rgb(u8, u8, u8);

struct TibiaMarkers {
    health_marker_one: Rgb,
    health_marker_two: Rgb,
    health_marker_three: Rgb,
    prev_attack_marker: Rgb,
    curr_attack_marker: Rgb,
}

impl TibiaMarkers {
    fn new() -> TibiaMarkers {
        TibiaMarkers {
            health_marker_one: FULL_GREEN,
            health_marker_two: FULL_GREEN,
            health_marker_three: FULL_GREEN,
            prev_attack_marker: ATTACK_AVAILABLE,
            curr_attack_marker: ATTACK_AVAILABLE,
        }
    }

    fn update_markers(&mut self, window: &Window) -> Result<()> {
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

        *self = TibiaMarkers {
            health_marker_one,
            health_marker_two,
            health_marker_three,
            prev_attack_marker: self.curr_attack_marker,
            curr_attack_marker: attack_marker,
        };

        Ok(())
    }
}

// based on an image capture determine which key needs pressing for auto healing
// if none then do nothing
fn auto_healing_task(markers: &TibiaMarkers) -> Option<Key> {
    match &markers.health_marker_one {
        &DEEP_RED | &RED_WINE | &RED => return Some(Key::F2), // exura vita
        &YELLOW => return Some(Key::F3),                      // exura gran
        _ => {}
    };

    // exura gran
    match &markers.health_marker_two {
        &GREENISH | &FULL_GREEN => {}
        _ => return Some(Key::F3), // exura gran
    };

    match &markers.health_marker_three {
        &GREENISH | &FULL_GREEN => None,
        _ => Some(Key::F4), // exura
    }
}

fn attack_cooldown_task(tibia_markers: &TibiaMarkers) {
    if tibia_markers.prev_attack_marker == ATTACK_IN_COOLDOWN
        && tibia_markers.curr_attack_marker == ATTACK_AVAILABLE
    {
        beep().unwrap();
    } else {
        sleep(Duration::from_millis(5));
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

    let tibia_markers = Arc::new(RwLock::new(TibiaMarkers::new()));

    // healing thread
    let tibia_markers_clone = Arc::clone(&tibia_markers);
    thread::spawn(move || {
        let mut enigo = Enigo::new();
        loop {
            if macos_get_active_window_app_name() != "Tibia" {
                sleep(Duration::from_secs(3));
                continue;
            }

            let tm = tibia_markers_clone.read().unwrap();

            match auto_healing_task(&tm) {
                Some(key) => {
                    enigo.key_click(key);
                    sleep(Duration::from_secs(1));
                }
                None => sleep(Duration::from_millis(5)),
            }
        }
    });

    // beep thread
    let tibia_markers_clone = Arc::clone(&tibia_markers);
    thread::spawn(move || loop {
        attack_cooldown_task(&tibia_markers_clone.read().unwrap());
    });

    // main thread will just update markers every 50ms
    loop {
        let tm = &mut tibia_markers.write().unwrap();
        tm.update_markers(&tibia_window)?;
        sleep(Duration::from_millis(5));
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

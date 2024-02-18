use anyhow::{anyhow, ensure, Context, Result};
use enigo::{Enigo, Key, KeyboardControllable};
use regex::Regex;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread::{self, sleep};
use std::time::Duration;
use xcap::Window;

// RGB colors for health marker
const FULL_GREEN: RGB = RGB(0, 137, 0);
const GREENISH: RGB = RGB(79, 114, 3);
const YELLOW: RGB = RGB(144, 110, 6);
const RED: RGB = RGB(137, 34, 34);
const DEEP_RED: RGB = RGB(137, 0, 0);
const RED_WINE: RGB = RGB(69, 0, 0);

// RGB colors for attack
// const ATTACK_AVAILABLE: RGB = RGB(56, 11, 0);
const ATTACK_IN_COOLDOWN: RGB = RGB(184, 38, 1);

#[derive(Copy, Clone, PartialEq)]
struct RGB(u8, u8, u8);

#[derive(Copy, Clone)]
struct TibiaMarkers {
    health_marker_one: RGB,
    health_marker_two: RGB,
    health_marker_three: RGB,
    attack_marker: RGB,
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
        let health_marker_one = RGB(r, g, b);

        // This is still greenish but exura is not enough
        // to heal the character to max level
        // x: 360 y: 58 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(720, 66)
            .context("could not get pixels on second health marker")?
            .0;
        let health_marker_two = RGB(r, g, b);

        // This marker is to exura when the char
        // receieves more than 50 damage.
        // x: 490 y: 58 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(980, 66)
            .context("could not get pixels on third health marker")?
            .0;
        let health_marker_three = RGB(r, g, b);

        // Attack marker to know when cooldown is over
        // x: 13 y: 786 (without DPI)
        let [r, g, b, _] = capture
            .get_pixel_checked(26, 1522)
            .context("could not get pixels on attack marker")?
            .0;
        let attack_marker = RGB(r, g, b);

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
fn auto_healing_task(markers: TibiaMarkers) -> Option<Key> {
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
        sleep(Duration::from_millis(1755));
        beep().unwrap();
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

    let (tx, rx) = mpsc::channel::<TibiaMarkers>();
    let (tx2, rx2) = mpsc::channel::<TibiaMarkers>();

    // healing thread
    thread::spawn(move || {
        let mut enigo = Enigo::new();
        loop {
            if !is_tibia_open() {
                sleep(Duration::from_secs(3));
                continue;
            }

            let mut latest_message = None;

            while let Ok(message) = rx.try_recv() {
                latest_message = Some(message);
            }

            if let Some(msg) = latest_message {
                if let Some(key) = auto_healing_task(msg) {
                    enigo.key_click(key);
                    sleep(Duration::from_secs(1));
                }
            } else {
                sleep(Duration::from_millis(50));
            }
        }
    });

    // attack beep thread
    thread::spawn(move || loop {
        let mut latest_tibia_markers: Option<TibiaMarkers> = None;

        while let Ok(tibia_markers_msg) = rx2.try_recv() {
            latest_tibia_markers = Some(tibia_markers_msg);
        }

        if let Some(tm) = latest_tibia_markers {
            attack_cooldown_task(tm);
        } else {
            sleep(Duration::from_millis(50));
        }
    });

    loop {
        let tibia_markers = TibiaMarkers::get_markers(&tibia_window)?;

        tx.send(tibia_markers)?;
        tx2.send(tibia_markers)?;

        sleep(Duration::from_millis(50));
    }
}

fn beep() -> Result<()> {
    Command::new("osascript").arg("-e").arg("beep").output()?;
    Ok(())
}

fn is_tibia_open() -> bool {
    if macos_get_active_window_app_name() == "Tibia" {
        true
    } else {
        false
    }
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
        .replace("\n", "");

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

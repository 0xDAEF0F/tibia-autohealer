use enigo::{Enigo, Key, KeyboardControllable};
use regex::Regex;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use xcap::Window;

const FULL_GREEN: (u8, u8, u8) = (0, 137, 0);
const GREENISH: (u8, u8, u8) = (79, 114, 3);
const YELLOW: (u8, u8, u8) = (144, 110, 6);
const RED: (u8, u8, u8) = (137, 34, 34);
const DEEP_RED: (u8, u8, u8) = (137, 0, 0);
const RED_WINE: (u8, u8, u8) = (69, 0, 0);

fn main() {
    let mut tries = 1;

    let windows = Window::all().expect("Could not retrieve the windows");
    let tibia_window = windows
        .iter()
        .find(|w| w.app_name() == "Tibia")
        .expect("Tibia should be open!");
    assert!(
        tibia_window.width() == 1440 && tibia_window.height() == 875,
        "Mismatch in window size. Please make it full size."
    );
    let mut enigo = Enigo::new();

    while tries <= 100 {
        let image_capture = tibia_window
            .capture_image()
            .expect("Was not able to capture screen in Tibia");

        // The first pixel of the health bar at the top.
        // It is used to check the color of the bar.
        let health_marker_one = image_capture
            .get_pixel_checked(24, 66)
            .expect("Pixel not on screen")
            .0;
        let health_marker_one: (u8, u8, u8) = (
            health_marker_one[0],
            health_marker_one[1],
            health_marker_one[2],
        );

        // This marker is to exura when the char
        // receieves more than 50 damage.
        let health_marker_two = image_capture
            .get_pixel_checked(980, 66)
            .expect("Pixel not on screen")
            .0;
        let health_marker_two: (u8, u8, u8) = (
            health_marker_two[0],
            health_marker_two[1],
            health_marker_two[2],
        );

        if !is_tibia_open() {
            println!("Tibia not opened. try #{tries}");
            tries += 1;
            continue;
        } else {
            tries = 1;
        }

        if health_marker_one == DEEP_RED || health_marker_one == RED_WINE {
            // exura vita
            enigo.key_click(Key::F2);
            sleep(Duration::from_secs(1));
        } else if health_marker_one == RED || health_marker_one == YELLOW {
            // exura gran
            enigo.key_click(Key::F3);
            sleep(Duration::from_secs(1));
        } else if !(health_marker_two == GREENISH || health_marker_two == FULL_GREEN) {
            // exura
            enigo.key_click(Key::F4);
            sleep(Duration::from_secs(1));
        } else {
            println!("All good.");
            sleep(Duration::from_millis(100));
        }
    }
}

fn is_tibia_open() -> bool {
    if macos_get_active_window_app_name() == "Tibia".to_string() {
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

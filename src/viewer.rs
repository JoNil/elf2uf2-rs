use minifb::{Key, Scale, Window, WindowOptions};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};

const WIDTH: usize = 360;
const HEIGHT: usize = 287;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Feature {
    pub x: i32,
    pub y: i32,
    pub val: i32,
}

pub fn run() -> (SyncSender<Vec<u8>>, SyncSender<Vec<Feature>>) {
    let (tx, rx) = sync_channel(2);
    let (feature_tx, feature_rx) = sync_channel(2);

    std::thread::spawn(move || {
        let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];

        let mut options = WindowOptions {
            scale: Scale::X4,
            ..Default::default()
        };

        let mut window = Window::new("Viewer - ESC to exit", WIDTH, HEIGHT, options)
            .unwrap_or_else(|e| {
                panic!("{}", e);
            });

        window.set_target_fps(60);

        let mut stride: i32 = 2 * 180;
        let mut offset: i32 = 0;
        let mut crosshair = true;

        let mut data = Vec::new();
        let mut fl: Vec<Feature> = Vec::new();

        while window.is_open() && !window.is_key_down(Key::Escape) {
            buffer.fill(0);

            match rx.try_recv() {
                Ok(new_data) => {
                    data = new_data;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                _ => (),
            }

            match feature_rx.try_recv() {
                Ok(new_fl) => {
                    fl = new_fl;
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                _ => (),
            }

            for (y, line) in data[offset as usize..].chunks(stride as usize).enumerate() {
                for (x, value) in line.iter().enumerate() {
                    if x + y * WIDTH < buffer.len() {
                        buffer[x + y * WIDTH] =
                            (*value as u32) << 16 | (*value as u32) << 8 | (*value as u32);
                    }
                }
            }

            if crosshair {
                for (x, y) in [
                    (WIDTH / 2, HEIGHT / 2),
                    (WIDTH / 2 + 1, HEIGHT / 2),
                    (WIDTH / 2 - 1, HEIGHT / 2),
                    (WIDTH / 2, HEIGHT / 2 - 1),
                    (WIDTH / 2, HEIGHT / 2 + 1),
                    (WIDTH / 2 + 2, HEIGHT / 2),
                    (WIDTH / 2 - 2, HEIGHT / 2),
                    (WIDTH / 2, HEIGHT / 2 - 2),
                    (WIDTH / 2, HEIGHT / 2 + 2),
                ] {
                    buffer[x + y * WIDTH] = 0x000ff000;
                }
            }

            for feature in &fl {
                let x = feature.x as usize;
                let y = feature.y as usize;

                if feature.val == -1 {
                    buffer[x + y * WIDTH] = 0x00ff0000;
                } else {
                    buffer[x + y * WIDTH] = 0x000000ff;
                }
            }

            if window.is_key_pressed(Key::S, minifb::KeyRepeat::Yes) {
                stride += 1;

                println!("Stride: {stride}, Offset: {offset}");
            }

            if window.is_key_pressed(Key::A, minifb::KeyRepeat::Yes) {
                stride = (stride - 1).max(1);

                println!("Stride: {stride}, Offset: {offset}");
            }

            if window.is_key_pressed(Key::F, minifb::KeyRepeat::Yes) {
                offset += 1;

                println!("Stride: {stride}, Offset: {offset}");
            }

            if window.is_key_pressed(Key::D, minifb::KeyRepeat::Yes) {
                offset = (offset - 1).max(0);

                println!("Stride: {stride}, Offset: {offset}");
            }

            if window.is_key_pressed(Key::K, minifb::KeyRepeat::No) {
                crosshair = !crosshair;
            }

            if window.is_key_pressed(Key::L, minifb::KeyRepeat::No) {
                if matches!(options.scale, Scale::X4) {
                    options.scale = Scale::X1;
                } else {
                    options.scale = Scale::X4;
                }

                window = Window::new("Viewer - ESC to exit", WIDTH, HEIGHT, options)
                    .unwrap_or_else(|e| {
                        panic!("{}", e);
                    });
            }

            window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        }
    });

    (tx, feature_tx)
}

use minifb::{Key, Scale, Window, WindowOptions};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};

const WIDTH: usize = 360;
const HEIGHT: usize = 287;

pub fn run() -> SyncSender<Vec<u8>> {
    let (tx, rx) = sync_channel(2);

    std::thread::spawn(move || {
        let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];

        let options = WindowOptions {
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

        let mut data = Vec::new();

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

            for (y, line) in data[offset as usize..].chunks(stride as usize).enumerate() {
                for (x, value) in line.iter().enumerate() {
                    if x + y * WIDTH < buffer.len() {
                        buffer[x + y * WIDTH] =
                            (*value as u32) << 16 | (*value as u32) << 8 | (*value as u32);
                    }
                }
            }

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

            window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        }
    });

    tx
}

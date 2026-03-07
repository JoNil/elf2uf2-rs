use minifb::{Key, Scale, Window, WindowOptions};
use std::{
    fs,
    sync::mpsc::{sync_channel, SyncSender, TryRecvError},
};

const WIDTH: usize = 360;
const HEIGHT: usize = 287;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Feature {
    pub x: i32,
    pub y: i32,
    pub val: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn run() -> (SyncSender<Vec<u8>>, SyncSender<Vec<Feature>>, SyncSender<Quaternion>) {
    let (tx, rx) = sync_channel(2);
    let (feature_tx, feature_rx) = sync_channel(2);
    let (orient_tx, orient_rx) = sync_channel::<Quaternion>(2);

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
        let mut orientation = Quaternion { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };
        let mut image_count = 0;
        let mut save_image = false;

        while window.is_open() && !window.is_key_down(Key::Escape) {
            buffer.fill(0);

            match rx.try_recv() {
                Ok(new_data) => {
                    data = new_data;

                    if save_image {
                        fs::write(
                            format!("C:/Users/LordJ/Desktop/images/{image_count}.bimg"),
                            &data,
                        )
                        .ok();
                        image_count += 1;
                    }
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

            match orient_rx.try_recv() {
                Ok(q) => {
                    orientation = q;
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
                let x = feature.x as i32;
                let y = feature.y as i32;

                if feature.val == -1 {
                    // Raw feature position: colored cross
                    let color = 0x00ff0000; // red
                    for d in -2i32..=2 {
                        if x + d >= 0 && (x + d) < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
                            buffer[(x + d) as usize + y as usize * WIDTH] = color;
                        }
                        if x >= 0 && x < WIDTH as i32 && y + d >= 0 && (y + d) < HEIGHT as i32 {
                            buffer[x as usize + (y + d) as usize * WIDTH] = color;
                        }
                    }
                } else {
                    // POI position: black dot
                    if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
                        buffer[x as usize + y as usize * WIDTH] = 0x00000000;
                    }
                }
            }

            // Draw 3D orientation widget in top-right corner
            {
                let cx = (WIDTH - 40) as i32;
                let cy = 40i32;
                let axis_len = 30.0f32;

                let axes: [(f32, f32, f32, u32); 3] = [
                    (1.0, 0.0, 0.0, 0x00ff0000), // X = red
                    (0.0, 1.0, 0.0, 0x0000ff00), // Y = green
                    (0.0, 0.0, 1.0, 0x000000ff), // Z = blue
                ];

                let q = &orientation;
                for (vx, vy, vz, color) in &axes {
                    // Rotate vector by quaternion: v' = q * v * q^-1
                    // Expanded formula for rotation of vector (vx,vy,vz) by quaternion (w,x,y,z):
                    let qw = q.w;
                    let qx = q.x;
                    let qy = q.y;
                    let qz = q.z;

                    let t0 = 2.0 * (qx * vx + qy * vy + qz * vz);
                    let t1 = qw * qw - (qx * qx + qy * qy + qz * qz);
                    let rx = t1 * vx + t0 * qx + 2.0 * qw * (qy * vz - qz * vy);
                    let ry = t1 * vy + t0 * qy + 2.0 * qw * (qz * vx - qx * vz);
                    let _rz = t1 * vz + t0 * qz + 2.0 * qw * (qx * vy - qy * vx);

                    // Orthographic projection: use X and Y, ignore Z
                    let ex = cx + (rx * axis_len) as i32;
                    let ey = cy - (ry * axis_len) as i32;

                    // Draw line from (cx,cy) to (ex,ey) using Bresenham's
                    let dx = (ex - cx).abs();
                    let dy = -(ey - cy).abs();
                    let sx = if cx < ex { 1i32 } else { -1 };
                    let sy = if cy < ey { 1i32 } else { -1 };
                    let mut err = dx + dy;
                    let mut x = cx;
                    let mut y = cy;
                    loop {
                        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
                            buffer[x as usize + y as usize * WIDTH] = *color;
                        }
                        if x == ex && y == ey {
                            break;
                        }
                        let e2 = 2 * err;
                        if e2 >= dy {
                            err += dy;
                            x += sx;
                        }
                        if e2 <= dx {
                            err += dx;
                            y += sy;
                        }
                    }
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

            save_image = window.is_key_down(Key::Space);

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

    (tx, feature_tx, orient_tx)
}

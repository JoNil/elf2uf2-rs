use minifb::{Key, Scale, Window, WindowOptions};
use std::sync::mpsc::{sync_channel, SyncSender, TryRecvError};

use image_tracker_rs::{BoundingBox, Feature};

const WIDTH: usize = 360;
const HEIGHT: usize = 287;
const SIZE: usize = WIDTH * HEIGHT;

pub fn run() -> SyncSender<Vec<u8>> {
    let (tx, rx) = sync_channel(2);

    std::thread::spawn(move || {
        let mut tc = image_tracker_rs::create_tracking_context_with_settings(5, 5, 5, false);
        let mut fl = [Feature::default(); 10];

        let mut bb =
            BoundingBox::from_center_width_height(WIDTH as i32 / 2, HEIGHT as i32 / 2, 32, 32);

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
        let mut last_data = Vec::new();

        while window.is_open() && !window.is_key_down(Key::Escape) {
            buffer.fill(0);

            match rx.try_recv() {
                Ok(new_data) => {
                    data = new_data;

                    if last_data.len() != data.len() {
                        image_tracker_rs::select_good_features(
                            &mut tc,
                            &data,
                            WIDTH as _,
                            HEIGHT as _,
                            &mut fl,
                            &bb,
                        );
                    } else {
                        image_tracker_rs::track_features::<SIZE>(
                            &mut tc,
                            &last_data,
                            &data,
                            WIDTH as _,
                            HEIGHT as _,
                            &mut fl,
                        );

                        let count = fl.iter().filter(|f| f.val >= 0).count();

                        let average_x = fl.iter().filter(|f| f.val >= 0).map(|f| f.x).sum::<f32>()
                            / count as f32;
                        let average_y = fl.iter().filter(|f| f.val >= 0).map(|f| f.y).sum::<f32>()
                            / count as f32;

                        for feature in &mut fl {
                            if (feature.x - average_x).abs() > 16.0
                                || (feature.y - average_y).abs() > 16.0
                            {
                                feature.x = -1.0;
                                feature.y = -1.0;
                                feature.val = -1;
                            }
                        }

                        let good_feature_count = fl.iter().filter(|f| f.val >= 0).count();

                        if good_feature_count > 2 {
                            bb = BoundingBox::from_center_width_height(
                                average_x as i32,
                                average_y as i32,
                                32,
                                32,
                            )
                            .clamp(WIDTH as _, HEIGHT as _);
                        }

                        image_tracker_rs::replace_lost_features(
                            &mut tc,
                            &data,
                            WIDTH as _,
                            HEIGHT as _,
                            &mut fl,
                            &bb,
                        );

                        let good_feature_count2 = fl.iter().filter(|f| f.val >= 0).count();

                        println!(
                            "{} {} {} {}",
                            bb.center_x(),
                            bb.center_y(),
                            good_feature_count,
                            good_feature_count2,
                        );

                        if good_feature_count2 < 2 {
                            bb = BoundingBox::from_center_width_height(
                                WIDTH as i32 / 2,
                                HEIGHT as i32 / 2,
                                32,
                                32,
                            );

                            image_tracker_rs::select_good_features(
                                &mut tc,
                                &data,
                                WIDTH as _,
                                HEIGHT as _,
                                &mut fl,
                                &bb,
                            );
                        }
                    }

                    last_data = data.clone();
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

            for feature in &fl {
                let x = feature.x as usize;
                let y = feature.y as usize;

                buffer[x + y * WIDTH] = 0x00ff0000;
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

            if window.is_key_pressed(Key::B, minifb::KeyRepeat::No) {
                fl.fill(Default::default());

                let bb = BoundingBox::from_center_width_height(
                    WIDTH as i32 / 2,
                    HEIGHT as i32 / 2,
                    32,
                    32,
                );

                image_tracker_rs::select_good_features(
                    &mut tc,
                    &data,
                    WIDTH as _,
                    HEIGHT as _,
                    &mut fl,
                    &bb,
                );
            }

            window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
        }
    });

    tx
}

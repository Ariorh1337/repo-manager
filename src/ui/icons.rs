use std::collections::HashMap;

const PULL_PNG: &[u8] = include_bytes!("../../assets/png/pull.png");
const PUSH_PNG: &[u8] = include_bytes!("../../assets/png/push.png");
const FOLDER_PNG: &[u8] = include_bytes!("../../assets/png/folder.png");
const EDIT_PNG: &[u8] = include_bytes!("../../assets/png/edit.png");
const TRASH_PNG: &[u8] = include_bytes!("../../assets/png/trash.png");
const REFRESH_PNG: &[u8] = include_bytes!("../../assets/png/refresh.png");
const CHECK_PNG: &[u8] = include_bytes!("../../assets/png/check.png");
const CROSS_PNG: &[u8] = include_bytes!("../../assets/png/cross.png");
const INFO_PNG: &[u8] = include_bytes!("../../assets/png/info.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconType {
    Pull,
    Push,
    Folder,
    Edit,
    Trash,
    Refresh,
    Check,
    Cross,
    Info,
}

impl IconType {
    pub fn png_data(self) -> &'static [u8] {
        match self {
            IconType::Pull => PULL_PNG,
            IconType::Push => PUSH_PNG,
            IconType::Folder => FOLDER_PNG,
            IconType::Edit => EDIT_PNG,
            IconType::Trash => TRASH_PNG,
            IconType::Refresh => REFRESH_PNG,
            IconType::Check => CHECK_PNG,
            IconType::Cross => CROSS_PNG,
            IconType::Info => INFO_PNG,
        }
    }
}

#[derive(Default)]
pub struct IconManager {
    loaded_icons: HashMap<IconType, egui::TextureHandle>,
}

impl IconManager {
    pub fn new() -> Self {
        Self {
            loaded_icons: HashMap::new(),
        }
    }

    pub fn get_icon(
        &mut self,
        ctx: &egui::Context,
        icon_type: IconType,
        _size: f32,
    ) -> egui::TextureHandle {
        if let Some(handle) = self.loaded_icons.get(&icon_type) {
            return handle.clone();
        }

        let png_data = icon_type.png_data();
        let texture_handle = self.load_png_as_texture(ctx, png_data, icon_type);

        self.loaded_icons.insert(icon_type, texture_handle.clone());
        texture_handle
    }

    fn load_png_as_texture(
        &self,
        ctx: &egui::Context,
        png_data: &[u8],
        icon_type: IconType,
    ) -> egui::TextureHandle {
        match image::load_from_memory(png_data) {
            Ok(img) => {
                let rgba_img = img.to_rgba8();
                let (width, height) = rgba_img.dimensions();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &rgba_img,
                );

                ctx.load_texture(
                    format!("{:?}_png", icon_type),
                    color_image,
                    egui::TextureOptions::default(),
                )
            }
            Err(e) => {
                println!(
                    "Failed to load PNG for {:?}: {}, using pixel art fallback",
                    icon_type, e
                );
                self.create_colored_fallback(ctx, 16.0, icon_type)
            }
        }
    }

    fn create_colored_fallback(
        &self,
        ctx: &egui::Context,
        size: f32,
        icon_type: IconType,
    ) -> egui::TextureHandle {
        let color = [80, 80, 80, 255];

        let size_usize = size as usize;
        let mut rgba_data = vec![0u8; size_usize * size_usize * 4];

        match icon_type {
            IconType::Trash => self.draw_trash_icon(&mut rgba_data, size_usize, color),
            IconType::Edit => self.draw_edit_icon(&mut rgba_data, size_usize, color),
            IconType::Pull => self.draw_pull_icon(&mut rgba_data, size_usize, color),
            IconType::Push => self.draw_push_icon(&mut rgba_data, size_usize, color),
            IconType::Refresh => self.draw_refresh_icon(&mut rgba_data, size_usize, color),
            IconType::Folder => self.draw_folder_icon(&mut rgba_data, size_usize, color),
            IconType::Check => self.draw_check_icon(&mut rgba_data, size_usize, color),
            IconType::Cross => self.draw_cross_icon(&mut rgba_data, size_usize, color),
            IconType::Info => self.draw_info_icon(&mut rgba_data, size_usize, color),
        }

        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([size_usize, size_usize], &rgba_data);
        ctx.load_texture(
            format!("{:?}_fallback", icon_type),
            color_image,
            egui::TextureOptions::default(),
        )
    }

    fn draw_trash_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw = (y >= 2 && y <= 4 && x >= 2 && x < size - 2)
                    || (y >= 5 && y < size - 2 && x >= 4 && x < size - 4)
                    || (y >= 5 && y < size - 2 && (x == 3 || x == size - 4))
                    || (y == size - 3 && x >= 3 && x < size - 3);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_edit_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw =
                    (x + y >= size - 3 && x + y <= size + 1) || (x <= 3 && y <= 3 && x + y <= 4);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_pull_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        let center = size / 2;
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw = (x == center && y >= 2 && y < size - 2)
                    || (y >= size - 5
                        && y < size - 2
                        && (x >= center - (y - (size - 5)) && x <= center + (y - (size - 5))));

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_push_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        let center = size / 2;
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw = (x == center && y >= 2 && y < size - 2)
                    || (y >= 2 && y <= 5 && (x >= center - (5 - y) && x <= center + (5 - y)));

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_refresh_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        let center = size / 2;
        let radius = size / 3;
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let dx = x as i32 - center as i32;
                let dy = y as i32 - center as i32;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                let should_draw = (dist >= radius as f32 - 1.0 && dist <= radius as f32 + 1.0)
                    || (x >= center + radius - 2
                        && x <= center + radius
                        && y >= center - 2
                        && y <= center + 2);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_folder_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw = (y >= size / 3 && y <= size / 3 + 2 && x >= 2 && x < size - 2)
                    || (y >= size / 3 + 3 && y < size - 2 && x >= 2 && x < size - 2)
                    || (y >= size / 3 && y < size - 2 && (x == 1 || x == size - 2))
                    || (y == size - 3 && x >= 1 && x < size - 1);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_check_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw =
                    (x >= size / 3 && y >= size / 2 && x + y >= size - 2 && x + y <= size + 2)
                        || (x <= size / 2
                            && y >= size / 3
                            && y <= size * 2 / 3
                            && (x + size - y).abs_diff(size) <= 2);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_cross_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let should_draw = (x.abs_diff(y) <= 1 && x >= 2 && x < size - 2)
                    || ((x + y).abs_diff(size - 1) <= 1 && x >= 2 && x < size - 2);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }

    fn draw_info_icon(&self, rgba_data: &mut [u8], size: usize, color: [u8; 4]) {
        let center = size / 2;
        let radius = size / 3;

        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;
                let dx = x as i32 - center as i32;
                let dy = y as i32 - center as i32;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();

                let should_draw = (dist >= radius as f32 - 1.0 && dist <= radius as f32 + 1.0)
                    || (x == center && y >= center - radius + 2 && y <= center - radius + 4)
                    || (x == center && y >= center - 2 && y <= center + radius - 3);

                if should_draw {
                    rgba_data[idx] = color[0];
                    rgba_data[idx + 1] = color[1];
                    rgba_data[idx + 2] = color[2];
                    rgba_data[idx + 3] = color[3];
                }
            }
        }
    }
}

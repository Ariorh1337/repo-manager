use super::icons::{IconManager, IconType};

// TODO: Добавить настройки размеров для UI элементов
pub struct UiSize {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
}

impl Default for UiSize {
    fn default() -> Self {
        Self {
            small: 12.0,
            medium: 16.0,
            large: 20.0,
        }
    }
}

pub struct Button;

impl Button {
    pub fn text<T: Into<String>>(text: T) -> ButtonBuilder {
        ButtonBuilder::new(ButtonContent::Text(text.into()))
    }

    pub fn icon(icon: IconType) -> ButtonBuilder {
        ButtonBuilder::new(ButtonContent::Icon(icon))
    }

    pub fn icon_text<T: Into<String>>(icon: IconType, text: T) -> ButtonBuilder {
        ButtonBuilder::new(ButtonContent::IconText(icon, text.into()))
    }

    pub fn text_icon<T: Into<String>>(text: T, icon: IconType) -> ButtonBuilder {
        ButtonBuilder::new(ButtonContent::TextIcon(text.into(), icon))
    }
}

#[derive(Debug, Clone)]
pub enum ButtonContent {
    Text(String),
    Icon(IconType),
    IconText(IconType, String),
    TextIcon(String, IconType),
}

pub struct ButtonBuilder {
    content: ButtonContent,
    size: Option<egui::Vec2>,
    icon_size: f32,
    style: ButtonStyle,
    full_width: bool,
}

#[derive(Debug, Clone)]
pub enum ButtonStyle {
    Default,
    Primary,
    Danger,
    Success,
}

impl ButtonBuilder {
    pub fn new(content: ButtonContent) -> Self {
        Self {
            content,
            size: None,
            icon_size: UiSize::default().small,
            style: ButtonStyle::Default,
            full_width: false,
        }
    }

    pub fn size(mut self, size: egui::Vec2) -> Self {
        self.size = Some(size);
        self
    }

    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = size;
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn primary(mut self) -> Self {
        self.style = ButtonStyle::Primary;
        self
    }

    pub fn danger(mut self) -> Self {
        self.style = ButtonStyle::Danger;
        self
    }

    pub fn success(mut self) -> Self {
        self.style = ButtonStyle::Success;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, icon_manager: &mut IconManager) -> egui::Response {
        let actual_size = self.size.unwrap_or_else(|| self.calculate_size(ui));
        let button_rect = egui::Rect::from_min_size(ui.cursor().min, actual_size);
        let response = ui.allocate_rect(button_rect, egui::Sense::click());

        let mut visuals = ui.style().interact(&response).clone();

        match self.style {
            ButtonStyle::Primary => {
                visuals.bg_fill = egui::Color32::from_rgb(70, 130, 255);
                if response.hovered() {
                    visuals.bg_fill = egui::Color32::from_rgb(90, 150, 255);
                }
            }
            ButtonStyle::Danger => {
                visuals.bg_fill = egui::Color32::from_rgb(220, 50, 50);
                if response.hovered() {
                    visuals.bg_fill = egui::Color32::from_rgb(240, 70, 70);
                }
            }
            ButtonStyle::Success => {
                visuals.bg_fill = egui::Color32::from_rgb(50, 180, 50);
                if response.hovered() {
                    visuals.bg_fill = egui::Color32::from_rgb(70, 200, 70);
                }
            }
            ButtonStyle::Default => {}
        }

        ui.painter()
            .rect_filled(button_rect, visuals.rounding, visuals.bg_fill);

        ui.painter()
            .rect_stroke(button_rect, visuals.rounding, visuals.bg_stroke);

        self.render_content(ui, icon_manager, button_rect, &visuals);

        response
    }

    fn calculate_size(&self, ui: &egui::Ui) -> egui::Vec2 {
        let padding = 8.0;
        let icon_text_spacing = 4.0;
        let min_height = 25.0;

        let base_size = match &self.content {
            ButtonContent::Text(text) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui
                    .fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE))
                    .size();
                egui::Vec2::new(
                    text_size.x + padding,
                    f32::max(min_height, text_size.y + padding),
                )
            }

            ButtonContent::Icon(_) => {
                let size = f32::max(min_height, self.icon_size + padding);
                egui::Vec2::new(size, size)
            }

            ButtonContent::IconText(_, text) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui
                    .fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE))
                    .size();
                let width = self.icon_size + icon_text_spacing + text_size.x + padding;
                let height = f32::max(min_height, f32::max(self.icon_size, text_size.y) + padding);
                egui::Vec2::new(width, height)
            }

            ButtonContent::TextIcon(text, _) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui
                    .fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE))
                    .size();
                let width = text_size.x + icon_text_spacing + self.icon_size + padding;
                let height = f32::max(min_height, f32::max(self.icon_size, text_size.y) + padding);
                egui::Vec2::new(width, height)
            }
        };

        if self.full_width {
            egui::Vec2::new(ui.available_width(), base_size.y)
        } else {
            base_size
        }
    }

    fn render_content(
        &self,
        ui: &mut egui::Ui,
        icon_manager: &mut IconManager,
        button_rect: egui::Rect,
        visuals: &egui::style::WidgetVisuals,
    ) {
        match &self.content {
            ButtonContent::Text(text) => {
                ui.painter().text(
                    button_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::TextStyle::Button.resolve(ui.style()),
                    visuals.text_color(),
                );
            }

            ButtonContent::Icon(icon_type) => {
                let texture = icon_manager.get_icon(ui.ctx(), *icon_type, self.icon_size);
                let icon_rect = egui::Rect::from_center_size(
                    button_rect.center(),
                    egui::Vec2::splat(self.icon_size),
                );

                ui.painter().image(
                    texture.id(),
                    icon_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }

            ButtonContent::IconText(icon_type, text) => {
                let icon_size = self.icon_size;
                let padding = 4.0;

                let icon_center = egui::Pos2::new(
                    button_rect.min.x + icon_size / 2.0 + 4.0,
                    button_rect.center().y,
                );
                let texture = icon_manager.get_icon(ui.ctx(), *icon_type, icon_size);
                let icon_rect =
                    egui::Rect::from_center_size(icon_center, egui::Vec2::splat(icon_size));

                ui.painter().image(
                    texture.id(),
                    icon_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                let text_pos = egui::Pos2::new(
                    button_rect.min.x + icon_size + padding + 4.0,
                    button_rect.center().y,
                );
                ui.painter().text(
                    text_pos,
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::TextStyle::Button.resolve(ui.style()),
                    visuals.text_color(),
                );
            }

            ButtonContent::TextIcon(text, icon_type) => {
                let icon_size = self.icon_size;
                let _padding = 4.0;

                let text_pos = egui::Pos2::new(button_rect.min.x + 4.0, button_rect.center().y);
                ui.painter().text(
                    text_pos,
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::TextStyle::Button.resolve(ui.style()),
                    visuals.text_color(),
                );

                let icon_center = egui::Pos2::new(
                    button_rect.max.x - icon_size / 2.0 - 4.0,
                    button_rect.center().y,
                );
                let texture = icon_manager.get_icon(ui.ctx(), *icon_type, icon_size);
                let icon_rect =
                    egui::Rect::from_center_size(icon_center, egui::Vec2::splat(icon_size));

                ui.painter().image(
                    texture.id(),
                    icon_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

pub struct Icon;

impl Icon {
    pub fn show(
        ui: &mut egui::Ui,
        icon_manager: &mut IconManager,
        icon_type: IconType,
        size: Option<f32>,
    ) {
        let icon_size = size.unwrap_or(UiSize::default().small);
        let texture = icon_manager.get_icon(ui.ctx(), icon_type, icon_size);
        let image = egui::Image::new(&texture).max_size(egui::Vec2::splat(icon_size));
        ui.add(image);
    }
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon_manager: &mut IconManager,
    icon: IconType,
) -> egui::Response {
    Button::icon(icon).show(ui, icon_manager)
}

pub fn icon_text_button<T: Into<String>>(
    ui: &mut egui::Ui,
    icon_manager: &mut IconManager,
    icon: IconType,
    text: T,
) -> egui::Response {
    Button::icon_text(icon, text).show(ui, icon_manager)
}

pub fn text_button<T: Into<String>>(
    ui: &mut egui::Ui,
    _icon_manager: &mut IconManager,
    text: T,
) -> egui::Response {
    ui.button(text.into())
}

pub fn icon_image(ui: &mut egui::Ui, icon_manager: &mut IconManager, icon: IconType) {
    Icon::show(ui, icon_manager, icon, None);
}

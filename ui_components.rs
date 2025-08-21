use crate::icons::{IconManager, IconType};

#[derive(Debug, Clone)]
pub enum ButtonContent {
    Text(String),
    Icon(IconType),
    IconText(IconType, String),
    TextIcon(String, IconType),
}

pub struct IconButton {
    content: ButtonContent,
    size: egui::Vec2,
    icon_size: f32,
}

impl IconButton {
    pub fn new(content: ButtonContent) -> Self {
        Self {
            content,
            size: egui::Vec2::new(80.0, 25.0),
            icon_size: 16.0,
        }
    }
    
    pub fn text<T: Into<String>>(text: T) -> Self {
        Self::new(ButtonContent::Text(text.into()))
    }
    
    pub fn icon(icon: IconType) -> Self {
        Self::new(ButtonContent::Icon(icon))
    }
    
    pub fn icon_text<T: Into<String>>(icon: IconType, text: T) -> Self {
        Self::new(ButtonContent::IconText(icon, text.into()))
    }
    
    pub fn text_icon<T: Into<String>>(text: T, icon: IconType) -> Self {
        Self::new(ButtonContent::TextIcon(text.into(), icon))
    }
    
    pub fn size(mut self, size: egui::Vec2) -> Self {
        self.size = size;
        self
    }
    
    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = size;
        self
    }
    
    fn calculate_size(&self, ui: &egui::Ui) -> egui::Vec2 {
        let padding = 8.0;
        let icon_text_spacing = 4.0;
        
        match &self.content {
            ButtonContent::Text(text) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui.fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE)).size();
                egui::Vec2::new(text_size.x + padding, f32::max(25.0, text_size.y + padding))
            }
            
            ButtonContent::Icon(_) => {
                egui::Vec2::new(self.icon_size + padding, f32::max(25.0, self.icon_size + padding))
            }
            
            ButtonContent::IconText(_, text) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui.fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE)).size();
                let width = self.icon_size + icon_text_spacing + text_size.x + padding;
                let height = f32::max(25.0, f32::max(self.icon_size, text_size.y) + padding);
                egui::Vec2::new(width, height)
            }
            
            ButtonContent::TextIcon(text, _) => {
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                let text_size = ui.fonts(|f| f.layout_no_wrap(text.clone(), font_id, egui::Color32::WHITE)).size();
                let width = text_size.x + icon_text_spacing + self.icon_size + padding;
                let height = f32::max(25.0, f32::max(self.icon_size, text_size.y) + padding);
                egui::Vec2::new(width, height)
            }
        }
    }
    
    pub fn show(self, ui: &mut egui::Ui, icon_manager: &mut IconManager) -> egui::Response {
        let actual_size = self.calculate_size(ui);
        let button_rect = egui::Rect::from_min_size(ui.cursor().min, actual_size);
        let response = ui.allocate_rect(button_rect, egui::Sense::click());
        
        let visuals = ui.style().interact(&response);
        let bg_color = if response.hovered() {
            visuals.bg_fill
        } else {
            ui.style().visuals.widgets.inactive.bg_fill
        };
        
        ui.painter().rect_filled(
            button_rect,
            visuals.rounding,
            bg_color,
        );
        
        ui.painter().rect_stroke(
            button_rect,
            visuals.rounding,
            visuals.bg_stroke,
        );
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
                let icon_rect = egui::Rect::from_center_size(icon_center, egui::Vec2::splat(icon_size));
                
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
                let padding = 4.0;
                
                let text_pos = egui::Pos2::new(
                    button_rect.min.x + 4.0,
                    button_rect.center().y,
                );
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
                let icon_rect = egui::Rect::from_center_size(icon_center, egui::Vec2::splat(icon_size));
                
                ui.painter().image(
                    texture.id(),
                    icon_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        
        response
    }
}

pub fn icon_button(ui: &mut egui::Ui, icon_manager: &mut IconManager, icon: IconType) -> egui::Response {
    let icon_size = 12.0;
    let texture = icon_manager.get_icon(ui.ctx(), icon, icon_size);
    
    let button_padding = ui.spacing().button_padding;
    let desired_size = egui::Vec2::new(
        icon_size + button_padding.x * 2.0,
        icon_size + button_padding.y * 2.0
    );
    
    let response = ui.allocate_response(desired_size, egui::Sense::click());
    let rect = response.rect;
    
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        
        ui.painter().rect_filled(rect, visuals.rounding, visuals.bg_fill);
        ui.painter().rect_stroke(rect, visuals.rounding, visuals.bg_stroke);
        
        let icon_rect = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(icon_size));
        ui.painter().image(texture.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
    
    response
}

pub fn icon_image(ui: &mut egui::Ui, icon_manager: &mut IconManager, icon: IconType) {
    let texture = icon_manager.get_icon(ui.ctx(), icon, 12.0);
    let image = egui::Image::new(&texture).max_size(egui::Vec2::splat(12.0));
    ui.add(image);
}

pub fn text_button<T: Into<String>>(ui: &mut egui::Ui, _icon_manager: &mut IconManager, text: T) -> egui::Response {
    ui.button(text.into())
}

pub fn icon_text_button<T: Into<String>>(ui: &mut egui::Ui, icon_manager: &mut IconManager, icon: IconType, text: T) -> egui::Response {
    let icon_size = 12.0;
    let texture = icon_manager.get_icon(ui.ctx(), icon, icon_size);
    let text_str = text.into();
    
    let font_id = egui::TextStyle::Button.resolve(ui.style());
    let text_galley = ui.fonts(|f| f.layout_no_wrap(text_str.clone(), font_id.clone(), ui.visuals().widgets.inactive.text_color()));
    let spacing = 4.0;
    
    let content_width = icon_size + spacing + text_galley.size().x;
    let content_height = text_galley.size().y.max(icon_size);
    
    let button_padding = ui.spacing().button_padding;
    let desired_size = egui::Vec2::new(
        content_width + button_padding.x * 2.0,
        content_height + button_padding.y * 2.0
    );
    
    let response = ui.allocate_response(desired_size, egui::Sense::click());
    let rect = response.rect;
    
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        
        ui.painter().rect_filled(rect, visuals.rounding, visuals.bg_fill);
        ui.painter().rect_stroke(rect, visuals.rounding, visuals.bg_stroke);
        
        let content_rect = rect.shrink2(button_padding);
        
        let total_content_width = icon_size + spacing + text_galley.size().x;
        let start_x = content_rect.center().x - total_content_width / 2.0;
        
        let icon_pos = egui::pos2(start_x, content_rect.center().y - icon_size / 2.0);
        let icon_rect = egui::Rect::from_min_size(icon_pos, egui::Vec2::splat(icon_size));
        ui.painter().image(texture.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        
        let text_pos = egui::pos2(start_x + icon_size + spacing, content_rect.center().y - text_galley.size().y / 2.0);
        ui.painter().galley(text_pos, text_galley, visuals.text_color());
    }
    
    response
}

pub fn text_icon_button<T: Into<String>>(ui: &mut egui::Ui, icon_manager: &mut IconManager, text: T, icon: IconType) -> egui::Response {
    let texture = icon_manager.get_icon(ui.ctx(), icon, 12.0);
    ui.horizontal(|ui| {
        let btn = ui.button(text.into());
        ui.add(egui::Image::new(&texture).max_size(egui::Vec2::splat(12.0)));
        btn
    }).inner
}

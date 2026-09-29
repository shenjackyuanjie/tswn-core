//! 深浅主题共用的语义配色，状态始终同时保留文字。

pub(super) fn hint(ui: &mut egui::Ui, text: &str) -> egui::Response { ui.label(egui::RichText::new(text).size(13.0)) }

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub info: egui::Color32,
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub emphasis: egui::Color32,
    pub purple: egui::Color32,
}

impl Palette {
    pub fn of(ui: &egui::Ui) -> Self {
        let rgb = egui::Color32::from_rgb;
        if ui.visuals().dark_mode {
            Self {
                info: rgb(117, 185, 255),
                success: rgb(113, 215, 161),
                warning: rgb(245, 194, 104),
                emphasis: rgb(255, 142, 150),
                purple: rgb(192, 166, 255),
            }
        } else {
            Self {
                info: rgb(24, 94, 170),
                success: rgb(24, 115, 73),
                warning: rgb(147, 87, 8),
                emphasis: rgb(180, 43, 56),
                purple: rgb(116, 69, 170),
            }
        }
    }

    pub fn status(self, status: &str) -> egui::Color32 {
        match status {
            "完成" => self.success,
            "已停止" | "停止中" => self.warning,
            "失败" => self.emphasis,
            _ => self.info,
        }
    }

    pub fn tool(self, tool: &str) -> egui::Color32 {
        match tool {
            "to-diy" => self.success,
            "namer-pf" => self.purple,
            "pair" => self.warning,
            _ => self.info,
        }
    }
}

use egui::pos2;
use egui::ColorImage;
use egui::Context;
use egui::Rect;
use walkers::sources::Attribution;
use walkers::Texture;
use walkers::TextureWithUv;
use walkers::TileId;
use walkers::Tiles;

pub struct LocalTiles {
    egui_ctx: Context,
}

impl LocalTiles {
    pub fn new(egui_ctx: Context) -> Self {
        Self { egui_ctx }
    }
}

impl Tiles for LocalTiles {
    fn at(&mut self, _tile_id: TileId) -> Option<TextureWithUv> {
        let image = ColorImage::example();

        Some(TextureWithUv {
            texture: Texture::from_color_image(image, &self.egui_ctx),
            uv: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        })
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Local rendering example",
            url: "https://github.com/podusowski/walkers",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        256
    }
}

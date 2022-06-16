use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{std140::AsStd140, *},
};

use crate::shader::{SimpleTextureMaterial, SimpleTextureSpec};

pub struct WoodMaterialPlugin;

pub type WoodMaterial = SimpleTextureMaterial<WoodMaterialSpec>;

impl Plugin for WoodMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<WoodMaterial>::default());
    }
}

#[derive(Clone, TypeUuid)]
#[uuid = "8f83afc2-8543-40d9-b8ec-fbdb11051ebf"]
pub struct WoodMaterialSpec {
    pub primary_color: Color,
    pub secondary_color: Color,
    pub hilight_color: Color,
    pub texture_offset: IVec2,
    pub size: UVec2,
    pub turns: usize,
    pub is_plank: bool,
    pub base_color_texture: Handle<Image>,
}

// 0.8 #[derive(ShaderType)]
#[derive(AsStd140)]
pub struct WoodMaterialUniformData {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    pub hilight_color: Vec4,
    pub texture_offset: IVec2,
    size: UVec2,
    turns: u32,
    is_plank: u32,
}

impl SimpleTextureSpec for WoodMaterialSpec {
    type Param = ();
    type Uniform = WoodMaterialUniformData;

    fn alpha_mode() -> AlphaMode {
        AlphaMode::Blend
    }
    fn texture_handle(&self) -> &Handle<Image> {
        &self.base_color_texture
    }
    fn sample_type() -> TextureSampleType {
        TextureSampleType::Uint
    }
    fn fragment_shader(asset_server: &AssetServer) -> Option<Handle<Shader>> {
        asset_server.watch_for_changes().unwrap();
        Some(asset_server.load("shaders/wood.wgsl"))
    }

    fn vertex_shader(asset_server: &AssetServer) -> Option<Handle<Shader>> {
        Some(asset_server.load("shaders/wood.wgsl"))
    }

    fn prepare_uniform_data(&self, _: &mut Self::Param) -> Option<Self::Uniform> {
        Some(WoodMaterialUniformData {
            primary_color: self.primary_color.as_linear_rgba_f32().into(),
            secondary_color: self.secondary_color.as_linear_rgba_f32().into(),
            texture_offset: self.texture_offset,
            turns: self.turns as u32,
            hilight_color: self.hilight_color.as_linear_rgba_f32().into(),
            size: self.size,
            is_plank: self.is_plank as u32,
        })
    }
}

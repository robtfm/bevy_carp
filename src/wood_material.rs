use bevy::{
    prelude::*, reflect::TypeUuid,
    render::render_resource::*,
};

use crate::shader::{SimpleTextureMaterial, SimpleTextureSpec};

pub struct WoodMaterialPlugin;

pub type WoodMaterial = SimpleTextureMaterial::<WoodMaterialSpec>;

impl Plugin for WoodMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<WoodMaterial>::default());
    }
}

#[derive(Clone, TypeUuid)]
#[uuid = "8f83afc2-8543-40d9-b8ec-fbdb11051ebf"]
pub struct WoodMaterialSpec {
    pub data: Vec4,
    pub hilight_color: Color,
    pub size: UVec2,
    pub is_plank: bool,
    pub base_color_texture: Handle<Image>,
}

#[derive(ShaderType)]
pub struct WoodMaterialUniformData {
    pub data: Vec4,
    pub hilight_color: Vec4,
    size: UVec2,
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
        Some(asset_server.load("shaders/wood.wgsl"))
    }

    fn vertex_shader(asset_server: &AssetServer) -> Option<Handle<Shader>> {
        Some(asset_server.load("shaders/wood.wgsl"))
    }

    fn prepare_uniform_data(&self, _: &mut Self::Param) -> Option<Self::Uniform> {
        Some(WoodMaterialUniformData {
            data: self.data,
            hilight_color: self.hilight_color.as_linear_rgba_f32().into(),
            size: self.size,
            is_plank: self.is_plank as u32,
        })
    }
}

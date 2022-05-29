#import bevy_pbr::mesh_view_bind_group
#import bevy_pbr::mesh_struct

struct WoodMaterial {
    data: vec4<f32>;
    hilight_color: vec4<f32>;
    size: vec2<u32>;
    is_plank: u32;
};
[[group(1), binding(0)]]
var<uniform> material: WoodMaterial;
[[group(1), binding(1)]]
var base_color_texture: texture_2d<u32>;
[[group(1), binding(2)]]
var base_color_sampler: sampler;

[[group(2), binding(0)]]
var<uniform> mesh: Mesh;

// ported from https://www.shadertoy.com/view/XssXRB
fn rand2(co: vec2<f32>, seed: f32) -> f32 {
	return fract(sin(dot(co.xy ,vec2<f32>(seed,78.233))) * 43758.5453);
}

fn rand(n: f32, seed: f32) -> f32 {
	return fract(sin(n*4532.63264)*5375.52465 * seed);
}

fn cos_interpolate(v1: f32, v2: f32, a: f32) -> f32 {
	let angle = a * 3.14159;
	let prc = (1.0 - cos(angle)) * 0.5;
	return v1 * (1.0 - prc) + v2 * prc;
}

fn noise(pos: f32, size: f32, seed: f32) -> f32 {	
	let grid = floor(pos * size) * 0.1;
    let pos_grid = ((pos) % (1.0/size)) * size;
	let next_grid =  floor((pos + (1.0/size)) * size) * 0.1;
	let sample1 = ((rand(grid, seed)));
	let sample2 = ((rand(next_grid, seed)));
	
    return cos_interpolate(sample1, sample2, pos_grid);
}
	

fn wood_texture(uv: vec3<f32>) -> vec3<f32>
{
	var u = noise(uv.x, 10.0, 272.0);
	u = u * noise(uv.y, 10.0, 272.0) ;
	u = u + noise(uv.y, 10.0, 272.0) ;
	
	let v = noise(uv.y + (u * 0.1), 110.0, 272.0);
		
	let val = u * v;
	let color_a = vec3<f32>(0.06, 0.03, 0.02) * 2.0;
	let color_b = vec3<f32>(0.09, 0.04, 0.03) * 0.8;

	return mix(color_a, color_b, val);
}

fn is_hole(xy: vec2<i32>) -> bool {
    let is_plank: bool = material.is_plank != 0u;
    let size = vec2<i32>(material.size);
    if (xy.x >= size.x || xy.x < 0 || xy.y >= size.y || xy.y < 0) {
        return true;
    }

    var in_set: bool = textureLoad(base_color_texture, xy, 0).r != 0u;

    return in_set != is_plank;
}

struct Vertex {
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
    [[location(2)]] uv: vec2<f32>;
};

struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] world_position: vec4<f32>;
    [[location(1)]] world_normal: vec3<f32>;
    [[location(2)]] uv: vec2<f32>;
};

struct FragmentInput {
    [[builtin(front_facing)]] is_front: bool;
    [[location(0)]] world_position: vec4<f32>;
    [[location(1)]] world_normal: vec3<f32>;
    [[location(2)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = mesh.model * vec4<f32>(vertex.position, 1.0);
    out.world_normal = mat3x3<f32>(
        mesh.inverse_transpose_model[0].xyz,
        mesh.inverse_transpose_model[1].xyz,
        mesh.inverse_transpose_model[2].xyz
    ) * vertex.normal;
    out.uv = vertex.uv;
    out.clip_position = view.view_proj * out.world_position;
    return out;
}

[[stage(fragment)]]
fn fragment(in: FragmentInput) -> [[location(0)]] vec4<f32> {
    var color = wood_texture(vec3<f32>(in.uv * 0.17 + material.data.rg * 20.0, 0.25));

    let pixel_uv = in.uv;
    let tile = vec2<i32>(pixel_uv);
    var alpha = 0.0;
    var hilight_alpha = 0.0;

    if (pixel_uv.x <= 0.0 || pixel_uv.y <= 0.0) {
        return vec4<f32>(0.0);
    }

    // if (pixel_uv.x <= 0.05 || pixel_uv.y <= 0.05) {
    //     return vec4<f32>(0.0, 1.0, 1.0, 1.0);
    // }
    // if (pixel_uv.x >= f32(material.size.x) - 0.05  || pixel_uv.y >= f32(material.size.y) - 0.05) {
    //     return vec4<f32>(0.0, 1.0, 1.0, 1.0);
    // }

    // if (pixel_uv.x < 1.0 && pixel_uv.y < 1.0) {
    //     return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    // }

    if (is_hole(tile)) {
        let is_hole_left = is_hole(tile + vec2<i32>(-1, 0));
        let is_hole_right = is_hole(tile + vec2<i32>(1, 0));
        let is_hole_up = is_hole(tile + vec2<i32>(0, -1));
        let is_hole_down = is_hole(tile + vec2<i32>(0, 1));

        let residual = fract(pixel_uv) - 0.5;
        var range = 0.2;
        var base = 0.45;

        var hilight = 0.1;
        if (material.is_plank == 1u) {
            base = 0.5;
            range = 0.05;
        }

        if (!is_hole_left) {
            let jag = range * noise(pixel_uv.y, 10.0, f32(tile.x));
            let distance = residual.x + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        } 
        
        if (!is_hole_right) {
            let jag = range * noise(pixel_uv.y, 10.0, f32(tile.x + 1));
            let distance = -residual.x + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        }

        if (!is_hole_up) {
            let jag = range * noise(pixel_uv.x, 10.0, f32(tile.y));
            let distance = residual.y + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        } 
        
        if (!is_hole_down) {
            let jag = range * noise(pixel_uv.x, 10.0, f32(tile.y + 1));
            let distance = -residual.y + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        }

        // corners
        for (var x = -1; x <= 1; x = x + 2) {
            for (var y = -1; y <= 1; y = y + 2) {
                if (!is_hole(tile + vec2<i32>(x, y))) {
                    var jag: f32;
                    let offset = residual * vec2<f32>(f32(x), f32(y)) - 0.5;
                    let x_ratio = (offset.x / (offset.x + offset.y));

                    jag = range * (noise(pixel_uv.x, 10.0, f32(tile.y + (y + 1) / 2)) * (1.0 - x_ratio) + 
                                    noise(pixel_uv.y, 10.0, f32(tile.x + (x + 1) / 2)) * x_ratio);

                    let distance = sqrt(dot(offset, offset)) - (0.5 - base) - jag;
                    if (distance < 0.0) {
                        alpha = 1.0;
                    } else {
                        hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
                    }
                }
            }
        }
    } else {
        let far = vec2<f32>(material.size) - pixel_uv;
        let min_uv = min(
            min(pixel_uv.x, pixel_uv.y),
            min(far.x, far.y));

        if (min_uv < 0.1) {
            return vec4<f32>(0.0, 0.0, 0.0, max(0.0, min_uv / 0.2));
        }

        alpha = 1.0;
    }

    if (alpha == 0.0) {
        return vec4<f32>(material.hilight_color.rgb, hilight_alpha);
    }

    return vec4<f32>(color, alpha);
}


#import bevy_pbr::mesh_view_bind_group
#import bevy_pbr::mesh_struct

struct WoodMaterial {
    primary_color: vec4<f32>;
    secondary_color: vec4<f32>;
    hilight_color: vec4<f32>;
    texture_offset: vec2<i32>;
    size: vec2<u32>;
    turns: u32;
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
	let sample1 = ((rand(grid, seed * 1.0 + 5.05)));
	let sample2 = ((rand(next_grid, seed * 1.0 + 5.05)));
	
    return cos_interpolate(sample1, sample2, pos_grid);
}
	

fn wood_texture(uv: vec3<f32>) -> vec3<f32>
{
	var u = noise(uv.x, 10.0, 272.0);
	u = u * noise(uv.y, 10.0, 272.0) ;
	u = u + noise(uv.y, 10.0, 272.0) ;
	
	let v = noise(uv.y + (u * 0.1), 110.0, 272.0);
		
	let val = u * v;
	let color_a = vec3<f32>(0.09, 0.04, 0.02) * 1.8;
	let color_b = vec3<f32>(0.14, 0.05, 0.03) * 0.6;

	return mix(color_a, color_b, val);
}

fn is_hole(xy: vec2<i32>) -> bool {
    let is_plank: bool = material.is_plank != 0u;
    let size = vec2<i32>(material.size);

    if (xy.x >= size.x || xy.x < 0 || xy.y >= size.y || xy.y < 0) {
        return is_plank;
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

    var tile_uv = in.uv;
    var texture_uv = in.uv - 0.5;
    var right = vec2<i32>(1, 0);
    var up = vec2<i32>(0, 1);

    for (var i: u32 = 0u; i<material.turns; i=i+1u) {
        tile_uv = vec2<f32>(tile_uv.y, -tile_uv.x);
        texture_uv = vec2<f32>(texture_uv.y, -texture_uv.x);
        right = vec2<i32>(-right.y, right.x);
        up = vec2<i32>(-up.y, up.x);
    }

    texture_uv = texture_uv + 0.5 + vec2<f32>(material.texture_offset);
    var color = wood_texture(vec3<f32>(texture_uv * 0.17, 0.25));

    let tile = vec2<i32>(in.uv);
    let material_tile = vec2<i32>(tile_uv) + material.texture_offset;

    var alpha = 0.0;
    var hilight_alpha = 0.0;

    if (is_hole(tile)) {
        let is_hole_left = is_hole(tile - right);
        let is_hole_right = is_hole(tile + right);
        let is_hole_up = is_hole(tile - up);
        let is_hole_down = is_hole(tile + up);

        let residual = (texture_uv - vec2<f32>(material_tile)) - 0.5;

        var range = 0.2;
        var base = 0.45;
        var size = 10.0;

        var hilight = 0.1;
        if (material.is_plank == 1u) {
            base = 0.5;
            range = 0.1;
            size = 1.5;
        }

        if (!is_hole_left) {
            let jag = range * (1.0 - noise(texture_uv.y, size, f32(material_tile.x)));
            let distance = residual.x + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        } 
        
        if (!is_hole_right) {
            let jag = range * noise(texture_uv.y, size, f32(material_tile.x+1));
            let distance = -residual.x + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        }

        if (!is_hole_up) {
            let jag = range * (1.0 - noise(texture_uv.x, size, f32(material_tile.y)));
            let distance = residual.y + base - jag;
            if (distance < 0.0) {
                alpha = 1.0;
            } else {
                hilight_alpha = max(hilight_alpha, material.hilight_color.w * (1.0 - distance / hilight));
            }
        } 
        
        if (!is_hole_down) {
            let jag = range * noise(texture_uv.x, size, f32(material_tile.y+1));
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
                if (!is_hole(tile + x * right + y * up) && (is_hole(tile + x * right) == is_hole(tile + y * up))) {
                    var jag: f32;
                    let offset = residual * vec2<f32>(f32(x), f32(y)) - 0.5;
                    let x_ratio = (abs(offset.x) / (abs(offset.x) + abs(offset.y)));

                    var x_noise = noise(texture_uv.x, size, (f32(material_tile.y) + f32(y) * 0.5 + 0.5));
                    var y_noise = noise(texture_uv.y, size, (f32(material_tile.x) + f32(x) * 0.5 + 0.5));
                    if (y < 0) { x_noise = 1.0 - x_noise; }
                    if (x < 0) { y_noise = 1.0 - y_noise; }

                    jag = range * cos_interpolate(y_noise, x_noise, 1.0-x_ratio);

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
        let far = vec2<f32>(material.size) - in.uv;
        let min_uv = min(
            min(in.uv.x, in.uv.y),
            min(far.x, far.y)
        );

        if (min_uv < 0.2) {
            return vec4<f32>(0.0, 0.0, 0.0, max(0.0, min_uv / 0.2));
        }

        alpha = 1.0;
    }

    if (alpha == 0.0) {
        return vec4<f32>(material.hilight_color.rgb, hilight_alpha);
    }

    return vec4<f32>(color, alpha);
}


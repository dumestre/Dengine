//! WGSL shaders for the Dengine renderer
//!
//! Professional Blinn-Phong lighting with vertex normals, texture support,
//! and grid rendering.

/// Main lit shader — Blinn-Phong with vertex normals
///
/// Vertex layout: position (vec3), normal (vec3), texcoord (vec2) = 32 bytes/vertex
///
/// Uniforms (bind group 0, binding 0):
///   - mvp: mat4x4<f32>           (64 bytes)
///   - model: mat4x4<f32>         (64 bytes)
///   - camera_pos: vec3<f32>      (12 bytes)
///   - light_intensity: f32       (4 bytes)
///   - light_dir: vec3<f32>       (12 bytes)
///   - light_enabled: f32         (4 bytes)
///   - light_color: vec3<f32>     (12 bytes)
///   - has_texture: f32           (4 bytes)
///   - tint: vec4<f32>            (16 bytes)
///   Total = 192 bytes
pub const LIT_SHADER: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    camera_pos: vec3<f32>,
    light_intensity: f32,
    light_dir: vec3<f32>,
    light_enabled: f32,
    light_color: vec3<f32>,
    has_texture: f32,
    tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> ubo: Uniforms;

@group(0) @binding(1)
var tex_sampler: sampler;

@group(0) @binding(2)
var albedo_texture: texture_2d<f32>;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = ubo.mvp * vec4<f32>(v.pos, 1.0);
    out.world_pos = (ubo.model * vec4<f32>(v.pos, 1.0)).xyz;
    // Transform normal by model matrix (assumes uniform scale or use inverse-transpose)
    out.world_normal = normalize((ubo.model * vec4<f32>(v.normal, 0.0)).xyz);
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    // Normalize interpolated normal
    var n = normalize(v.world_normal);

    // If normal is degenerate, compute from derivatives as fallback
    let n_len = dot(n, n);
    if (n_len < 0.01) {
        let dpdx_pos = dpdx(v.world_pos);
        let dpdy_pos = dpdy(v.world_pos);
        let c = cross(dpdx_pos, dpdy_pos);
        let len2 = max(dot(c, c), 1e-8);
        n = c * inverseSqrt(len2);
    }

    // View direction
    let view_dir = normalize(ubo.camera_pos - v.world_pos);

    // Default lighting factors
    let ambient = 0.15;
    var diffuse = 0.0;
    var specular = 0.0;
    var l_color = vec3<f32>(1.0, 1.0, 1.0);

    if (ubo.light_enabled > 0.5) {
        // Light direction (normalized directional light)
        let l = normalize(ubo.light_dir);
        let ndotl = max(dot(n, l), 0.0);
        diffuse = ndotl * 0.65;

        // Specular (Blinn half-vector)
        let half_dir = normalize(l + view_dir);
        let ndoth = max(dot(n, half_dir), 0.0);
        specular = pow(ndoth, 32.0) * 0.25;

        l_color = ubo.light_color * ubo.light_intensity;
    }

    // Rim lighting for depth perception
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 2.5) * 0.08;

    // Fill light from below-behind for shadow areas
    let fill_dir = normalize(vec3<f32>(-0.3, -0.5, -0.4));
    let fill = max(dot(n, fill_dir), 0.0) * 0.10;

    let total_light = (ambient + (diffuse + specular) + rim + fill);
    let shade = clamp(total_light, 0.0, 1.5); // Allow slightly over 1.0 for highlights

    // Base color from texture or tint
    var base_color = ubo.tint;
    if (ubo.has_texture > 0.5) {
        let tex_color = textureSample(albedo_texture, tex_sampler, v.uv);
        base_color = tex_color * ubo.tint;
    }

    let color = base_color.rgb * l_color * shade;
    return vec4<f32>(color, base_color.a);
}
"#;

/// Uniform buffer size in bytes (must match the Uniforms struct above)
pub const LIT_UNIFORM_SIZE: usize = 192;

/// Stride of a single vertex in bytes: pos(12) + normal(12) + uv(8) = 32
pub const LIT_VERTEX_STRIDE: usize = 32;

/// Grid shader — infinite ground grid rendered via fullscreen quad
pub const GRID_SHADER: &str = r#"
struct GridUniforms {
    view_proj_inv: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0)
var<uniform> grid_ubo: GridUniforms;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

fn unproject(p: vec3<f32>) -> vec3<f32> {
    let r = grid_ubo.view_proj_inv * vec4<f32>(p, 1.0);
    return r.xyz / r.w;
}

@vertex
fn vs_grid(@builtin(vertex_index) idx: u32) -> VsOut {
    // Fullscreen triangle
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let p = positions[idx];
    var out: VsOut;
    out.clip_pos = vec4<f32>(p, 0.0, 1.0);
    out.near_point = unproject(vec3<f32>(p, 0.0));
    out.far_point = unproject(vec3<f32>(p, 1.0));
    return out;
}

@fragment
fn fs_grid(v: VsOut) -> @location(0) vec4<f32> {
    let t = -v.near_point.y / (v.far_point.y - v.near_point.y);
    if (t < 0.0) { discard; }
    let world_pos = v.near_point + t * (v.far_point - v.near_point);

    let coord = world_pos.xz;
    let grid = abs(fract(coord - 0.5) - 0.5) / fwidth(coord);
    let line = min(grid.x, grid.y);
    let alpha = 1.0 - min(line, 1.0);

    // Fade with distance
    let dist = length(world_pos.xz - grid_ubo.camera_pos.xz);
    let fade = 1.0 - smoothstep(8.0, 40.0, dist);

    let color = vec3<f32>(0.35, 0.35, 0.38);
    return vec4<f32>(color, alpha * fade * 0.4);
}
"#;

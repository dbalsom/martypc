precision highp float;
precision highp int;

in vec2 v_uv;

uniform sampler2D u_texture;
uniform float u_h_curvature;
uniform float u_v_curvature;
uniform float u_corner_radius;
uniform int u_scanlines;
uniform float u_gamma;
uniform int u_mono;
uniform vec4 u_mono_color;
uniform int u_vres;
uniform int u_texture_order;
uniform int u_crtc_frame_parity;
uniform int u_crtc_interlaced;
uniform int u_crtc_interlace_support;
uniform float u_power_off;

out vec4 out_color;

const float PI = 3.141592653589793;
const float POWER_OFF_VERTICAL_COLLAPSE_SPEED = 2.85;
const float POWER_OFF_HORIZONTAL_COLLAPSE_SPEED = 1.5;
const float POWER_OFF_HORIZONTAL_COLLAPSE_DELAY = 0.25;

float brightness(vec4 color) {
    return 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
}

vec2 apply_crt_curvature(vec2 uv) {
    float curvature_x = u_h_curvature * 0.1;
    float curvature_y = u_v_curvature * 0.1;

    // Remap UV from [0,1] to [-1,1].
    vec2 uv_mapped = uv * 2.0 - 1.0;
    float radius_squared = uv_mapped.x * uv_mapped.x + uv_mapped.y * uv_mapped.y;
    float distortion = 1.0 - radius_squared * (curvature_x + curvature_y);
    uv_mapped /= distortion;

    return uv_mapped * 0.5 + 0.5;
}

bool is_inside_corner_radius(vec2 uv, float corner_radius) {
    vec2 uv_radius = vec2(corner_radius);

    vec2 top_left_center = uv_radius;
    vec2 top_right_center = vec2(1.0 - uv_radius.x, uv_radius.y);
    vec2 bottom_left_center = vec2(uv_radius.x, 1.0 - uv_radius.y);
    vec2 bottom_right_center = vec2(1.0) - uv_radius;

    bool left_side = uv.x < uv_radius.x;
    bool right_side = uv.x > 1.0 - uv_radius.x;
    bool top_side = uv.y < uv_radius.y;
    bool bottom_side = uv.y > 1.0 - uv_radius.y;

    bool in_top_left_corner = left_side && top_side && distance(uv, top_left_center) > corner_radius;
    bool in_top_right_corner = right_side && top_side && distance(uv, top_right_center) > corner_radius;
    bool in_bottom_left_corner = left_side && bottom_side && distance(uv, bottom_left_center) > corner_radius;
    bool in_bottom_right_corner = right_side && bottom_side && distance(uv, bottom_right_center) > corner_radius;

    return !(in_top_left_corner || in_top_right_corner || in_bottom_left_corner || in_bottom_right_corner);
}

vec4 do_monochrome(vec4 color, float gamma) {
    float level = max(brightness(color), 0.0);
    return u_mono_color * pow(abs(level), gamma);
}

vec4 do_scanlines(vec4 color, float y_coord, float texture_lines, float lines, float intensity) {
    float factor = 1.0 - intensity;
    float texel_pos = y_coord * texture_lines - 0.5;
    float line_pos = texel_pos * (lines / texture_lines);
    float phosphor = 0.5 + 0.5 * cos(line_pos * 2.0 * PI);
    float scanline_effect = mix(factor, 1.0, phosphor);

    return vec4(color.rgb * scanline_effect, color.a);
}

void main() {
    float power_off = clamp(u_power_off, 0.0, 1.0);
    vec2 effect_tex_coord = v_uv;
    float power_mask = 1.0;
    float vertical_collapse = 0.0;

    if (power_off > 0.0) {
        // Collapse the picture vertically until its entire contents occupy a thin horizontal line.
        float vertical_time = clamp(power_off * POWER_OFF_VERTICAL_COLLAPSE_SPEED, 0.0, 1.0);
        vertical_collapse = (1.0 - exp(-3.0 * vertical_time)) / (1.0 - exp(-3.0));
        float half_height = mix(0.5, 0.0025, vertical_collapse);
        float vertical_edge_softness = mix(0.002, 0.005, vertical_collapse);
        float vertical_center_distance = abs(v_uv.y - 0.5);
        power_mask = 1.0 - smoothstep(
            half_height,
            half_height + vertical_edge_softness,
            vertical_center_distance
        );
        effect_tex_coord.y = clamp(0.5 + (v_uv.y - 0.5) / (half_height * 2.0), 0.0, 1.0);

        // Collapse horizontally after a tunable delay, with an independently tunable speed.
        float horizontal_time = clamp(
            (power_off - POWER_OFF_HORIZONTAL_COLLAPSE_DELAY) * POWER_OFF_HORIZONTAL_COLLAPSE_SPEED,
            0.0,
            1.0
        );
        float horizontal_collapse = (1.0 - exp(-3.0 * horizontal_time)) / (1.0 - exp(-3.0));
        float half_width = mix(0.5, 0.0025, horizontal_collapse);
        float horizontal_edge_softness = mix(0.002, 0.005, horizontal_collapse);
        float horizontal_center_distance = abs(v_uv.x - 0.5);
        power_mask *= 1.0 - smoothstep(
            half_width,
            half_width + horizontal_edge_softness,
            horizontal_center_distance
        );
        effect_tex_coord.x = clamp(0.5 + (v_uv.x - 0.5) / (half_width * 2.0), 0.0, 1.0);
    }

    // Keep containment tied to the original scaler geometry. Only texture sampling is animated.
    vec2 containment_tex_coord = apply_crt_curvature(v_uv);
    vec2 curved_tex_coord = apply_crt_curvature(effect_tex_coord);
    bool is_outside = any(lessThan(containment_tex_coord, vec2(0.0)))
        || any(greaterThan(containment_tex_coord, vec2(1.0)));
    bool is_inside_corner = is_inside_corner_radius(containment_tex_coord, u_corner_radius * 0.1);

    bool interlace_shift_enabled = u_crtc_interlace_support != 0
        && u_crtc_interlaced != 0
        && u_crtc_frame_parity == 1;
    float parity_shift = interlace_shift_enabled ? 0.5 / max(float(u_vres), 1.0) : 0.0;
    vec2 sampled_tex_coord = vec2(curved_tex_coord.x, curved_tex_coord.y - parity_shift);

    vec4 color = texture(u_texture, sampled_tex_coord);
    if (u_texture_order != 0) {
        color = color.bgra;
    }

    if (is_outside || !is_inside_corner) {
        discard;
    }
    else {
        if (u_mono != 0) {
            color = do_monochrome(color, u_gamma);
        }

        if (u_scanlines > 0) {
            float scanline_phase_shift = interlace_shift_enabled
                ? 0.5 / max(float(u_scanlines), 1.0)
                : 0.0;
            color = do_scanlines(
                color,
                curved_tex_coord.y - scanline_phase_shift,
                float(u_vres),
                float(u_scanlines),
                0.3
            );
        }
    }

    if (power_off > 0.0) {
        // Overdrive the source image as it collapses
        float gamma = mix(1.0, 0.55, vertical_collapse);
        float contrast = mix(1.0, 1.35, vertical_collapse);
        float exposure = mix(1.0, 4.0, vertical_collapse);

        vec3 gamma_color = pow(max(color.rgb, vec3(0.0)), vec3(gamma));
        vec3 contrast_color = max((gamma_color - 0.5) * contrast + 0.5, vec3(0.0));

        float white_point = 1.0 - exp(-exposure);
        vec3 energized_color = (vec3(1.0) - exp(-contrast_color * exposure)) / white_point;
        float vertical_duration = 1.0 / POWER_OFF_VERTICAL_COLLAPSE_SPEED;
        float fade = 1.0 - smoothstep(vertical_duration, 1.0, power_off);

        color = vec4(mix(color.rgb, energized_color, vertical_collapse) * fade * power_mask, 1.0);
    }

    // Override alpha for glow
    out_color = vec4(color.rgb, 1.0);
}

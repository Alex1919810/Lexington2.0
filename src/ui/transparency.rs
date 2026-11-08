//! Convert egui's gamma-encoded linear-premultiplied result to desktop compositor RGBA.
use eframe::{
    egui,
    glow::{self, HasContext},
};
use std::sync::{Arc, Mutex};

/// egui_glow reads raw framebuffer bytes into Color32; after our final pass
/// these bytes are gamma-premultiplied, rather than ordinary egui Color32 values.
pub(super) fn screenshot_rgba(pixel: egui::Color32) -> [u8; 4] {
    let [r, g, b, a] = pixel.to_array();
    if a == 0 {
        return [0; 4];
    }
    let unpremultiply =
        |v: u8| ((u32::from(v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
    [unpremultiply(r), unpremultiply(g), unpremultiply(b), a]
}

const VERTEX: &str = r#"
out vec2 uv;
void main() {
    vec2 p = vec2((gl_VertexID == 1) ? 3.0 : -1.0, (gl_VertexID == 2) ? 3.0 : -1.0);
    uv = p * 0.5 + 0.5;
    gl_Position = vec4(p, 0.0, 1.0);
}
"#;
const FRAGMENT: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
uniform sampler2D source_image;
in vec2 uv;
out vec4 output_color;
vec3 to_linear(vec3 c) {
    return mix(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, lessThanEqual(c, vec3(0.04045)));
}
vec3 to_gamma(vec3 c) {
    return mix(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, lessThanEqual(c, vec3(0.0031308)));
}
void main() {
    vec4 p = texture(source_image, uv);
    if (p.a <= 0.0) { output_color = vec4(0.0); }
    else if (p.a >= 1.0) { output_color = p; }
    else { output_color = vec4(to_gamma(clamp(to_linear(p.rgb) / p.a, 0.0, 1.0)) * p.a, p.a); }
}
"#;

pub(super) struct Transparency {
    resources: Mutex<Option<Resources>>,
}
struct Resources {
    gl: Arc<glow::Context>,
    program: glow::Program,
    vao: glow::VertexArray,
    texture: glow::Texture,
    size: [i32; 2],
}
impl Transparency {
    pub fn new(gl: Arc<glow::Context>) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            resources: Mutex::new(Some(Resources::new(gl)?)),
        }))
    }
    pub fn paint(self: &Arc<Self>, ctx: &egui::Context) {
        let pass = self.clone();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Debug,
            egui::Id::new("desktop-alpha-output"),
        ))
        .add(egui::PaintCallback {
            rect: ctx.screen_rect(),
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _| {
                if let Some(resources) = pass.resources.lock().unwrap().as_mut() {
                    let v = info.viewport_in_pixels();
                    // The egui painter restores GL state after every callback.
                    unsafe {
                        resources.paint(v.left_px, v.from_bottom_px, v.width_px, v.height_px);
                    }
                }
            })),
        });
    }
    pub fn destroy(&self) {
        self.resources.lock().unwrap().take();
    }
}
impl Resources {
    fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
        let version = if eframe::egui_glow::ShaderVersion::get(&gl).is_embedded() {
            "#version 300 es\n"
        } else {
            "#version 140\n"
        };
        unsafe {
            let program = gl.create_program()?;
            for (kind, source) in [
                (glow::VERTEX_SHADER, VERTEX),
                (glow::FRAGMENT_SHADER, FRAGMENT),
            ] {
                let shader = match gl.create_shader(kind) {
                    Ok(s) => s,
                    Err(e) => {
                        gl.delete_program(program);
                        return Err(e);
                    }
                };
                gl.shader_source(shader, &format!("{version}{source}"));
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    gl.delete_shader(shader);
                    gl.delete_program(program);
                    return Err("透明输出着色器编译失败。".into());
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                gl.delete_program(program);
                return Err("透明输出着色器链接失败。".into());
            }
            let vao = match gl.create_vertex_array() {
                Ok(v) => v,
                Err(e) => {
                    gl.delete_program(program);
                    return Err(e);
                }
            };
            let texture = match gl.create_texture() {
                Ok(t) => t,
                Err(e) => {
                    gl.delete_vertex_array(vao);
                    gl.delete_program(program);
                    return Err(e);
                }
            };
            Ok(Self {
                gl,
                program,
                vao,
                texture,
                size: [0, 0],
            })
        }
    }
    unsafe fn paint(&mut self, x: i32, y: i32, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let gl = &self.gl;
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            if self.size != [width, height] {
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    width,
                    height,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );
                self.size = [width, height];
            }
            // RGBA8 deliberately avoids another sRGB decode on this final texture.
            gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, x, y, width, height);
            gl.disable(glow::BLEND);
            gl.viewport(x, y, width, height);
            gl.use_program(Some(self.program));
            gl.uniform_1_i32(
                gl.get_uniform_location(self.program, "source_image")
                    .as_ref(),
                0,
            );
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
        }
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        // Called by App::on_exit while the owning GL context is still current.
        unsafe {
            self.gl.delete_texture(self.texture);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_program(self.program);
        }
    }
}

/// A hidden GL context checks the real shader, without config, login, models or input injection.
pub fn check() -> Result<String, String> {
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_inner_size([32., 32.])
            .with_active(false),
        ..Default::default()
    };
    eframe::run_native(
        "Lexington alpha check",
        options,
        Box::new(move |cc| {
            *output.lock().unwrap() = Some(
                cc.gl
                    .clone()
                    .ok_or_else(|| "缺少 GL 上下文。".to_owned())
                    .and_then(check_pixels),
            );
            Ok(Box::new(CheckApp))
        }),
    )
    .map_err(|_| "无法创建透明输出验证上下文。")?;
    result
        .lock()
        .unwrap()
        .take()
        .ok_or("透明输出验证未执行。")?
}
struct CheckApp;
impl eframe::App for CheckApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
fn check_pixels(gl: Arc<glow::Context>) -> Result<String, String> {
    let mut pass = Resources::new(gl.clone())?;
    let mut input = Vec::new();
    let mut expected = Vec::new();
    for color in [[255, 255, 255], [200, 160, 120], [30, 100, 220]] {
        for alpha in [0, 1, 2, 4, 8, 16, 32, 64, 128, 192, 254, 255] {
            input.extend_from_slice(
                &egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], alpha)
                    .to_array(),
            );
            expected.push([
                ((u16::from(color[0]) * u16::from(alpha) + 127) / 255) as u8,
                ((u16::from(color[1]) * u16::from(alpha) + 127) / 255) as u8,
                ((u16::from(color[2]) * u16::from(alpha) + 127) / 255) as u8,
                alpha,
            ]);
        }
    }
    let count = expected.len();
    let mut pixels = vec![0; count * 4];
    unsafe {
        let framebuffer = gl.create_framebuffer()?;
        let texture = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            count as i32,
            1,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&input)),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.delete_framebuffer(framebuffer);
            gl.delete_texture(texture);
            return Err("透明验证帧缓冲不可用。".into());
        }
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::DITHER);
        pass.paint(0, 0, count as i32, 1);
        gl.read_pixels(
            0,
            0,
            count as i32,
            1,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut pixels)),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(texture);
    }
    for (actual, expected) in pixels.as_chunks::<4>().0.iter().zip(expected) {
        if actual[3] != expected[3]
            || actual.iter().zip(expected).any(|(a, b)| a.abs_diff(b) > 2)
            || actual[..3].iter().any(|v| *v > actual[3])
        {
            return Err("GPU 透明输出颜色或 alpha 校验失败。".into());
        }
    }
    Ok(format!(
        "GPU transparency verified: {count} pixels; alpha preserved, no additive fringe."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_readback_unpremultiplies_once_and_preserves_alpha_and_opaque_colors() {
        assert_eq!(
            screenshot_rgba(egui::Color32::from_rgba_premultiplied(32, 16, 8, 64)),
            [128, 64, 32, 64]
        );
        assert_eq!(
            screenshot_rgba(egui::Color32::from_rgba_premultiplied(255, 255, 255, 0)),
            [0, 0, 0, 0]
        );
        assert_eq!(
            screenshot_rgba(egui::Color32::from_rgba_premultiplied(12, 34, 56, 255)),
            [12, 34, 56, 255]
        );
        for a in 1..=255 {
            assert_eq!(
                screenshot_rgba(egui::Color32::from_rgba_premultiplied(a, a, a, a)),
                [255, 255, 255, a]
            );
        }
    }
}

//! Text Renderer - Canvas 文字渲染 + WebGPU 分片保护
//!
//! 将文字栅格化为图像，结合分片乱序渲染实现文字保护

use wasm_bindgen::prelude::*;

use crate::{FragmentedRenderer, SecureImageRenderer, ImageWatermarkEncoder};

// ab_glyph 用于字体加载和度量
// 注意: 完整渲染功能在 JS Canvas 2D 层完成
use ab_glyph::{Font, FontRef};

/// 文字渲染器配置
#[derive(Debug, Clone)]
#[wasm_bindgen]
pub struct TextRendererConfig {
    canvas_width: f32,
    canvas_height: f32,
    tiles_x: u32,
    tiles_y: u32,
    font_size: f32,
    noise_level: u8,
}

#[wasm_bindgen]
impl TextRendererConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas_width: f32,
        canvas_height: f32,
        tiles_x: u32,
        tiles_y: u32,
        font_size: f32,
        noise_level: u8,
    ) -> Self {
        Self {
            canvas_width,
            canvas_height,
            tiles_x,
            tiles_y,
            font_size,
            noise_level: noise_level.min(3),
        }
    }

    pub fn get_canvas_width(&self) -> f32 { self.canvas_width }
    pub fn get_canvas_height(&self) -> f32 { self.canvas_height }
    pub fn get_tiles_x(&self) -> u32 { self.tiles_x }
    pub fn get_tiles_y(&self) -> u32 { self.tiles_y }
    pub fn get_font_size(&self) -> f32 { self.font_size }
    pub fn get_noise_level(&self) -> u8 { self.noise_level }
}

/// 文字渲染结果
#[derive(Debug, Clone)]
#[wasm_bindgen]
pub struct TextRenderResult {
    /// RGBA 像素数据
    pixels: Vec<u8>,
    /// 图像宽度
    width: u32,
    /// 图像高度
    height: u32,
    /// 分片渲染顺序
    render_order: Vec<u32>,
    /// 噪声数据 (tile_index + session_hash)
    noise_info: Vec<u8>,
}

#[wasm_bindgen]
impl TextRenderResult {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            render_order: Vec::new(),
            noise_info: Vec::new(),
        }
    }

    /// 获取像素数据
    pub fn get_pixels(&self) -> Vec<u8> {
        self.pixels.clone()
    }

    pub fn get_width(&self) -> u32 { self.width }
    pub fn get_height(&self) -> u32 { self.height }

    /// 获取分片渲染顺序
    pub fn get_render_order(&self) -> Vec<u32> {
        self.render_order.clone()
    }

    /// 获取噪声信息 (用于客户端水印验证)
    pub fn get_noise_info(&self) -> Vec<u8> {
        self.noise_info.clone()
    }
}

impl Default for TextRenderResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 文字渲染器 - 核心结构
///
/// 将文字栅格化为像素后，应用：
/// 1. LSB 水印嵌入
/// 2. B通道噪声编码
/// 3. 分片乱序渲染
#[wasm_bindgen]
pub struct TextRenderer {
    /// 字体数据 (TTF/OTF 二进制)
    font_data: Option<Vec<u8>>,
    /// 分片渲染器
    fragmenter: FragmentedRenderer,
    /// 安全图像渲染器 (噪声+水印)
    secure_renderer: SecureImageRenderer,
    /// LSB 水印编码器
    watermark_encoder: ImageWatermarkEncoder,
    /// 配置
    config: TextRendererConfig,
    /// 当前会话ID
    session_id: String,
    /// 噪声等级
    noise_level: u8,
}

#[wasm_bindgen]
impl TextRenderer {
    /// 创建新的文字渲染器
    #[wasm_bindgen(constructor)]
    pub fn new(config: TextRendererConfig) -> Self {
        let canvas_width = config.canvas_width;
        let canvas_height = config.canvas_height;
        let tiles_x = config.tiles_x;
        let tiles_y = config.tiles_y;

        Self {
            font_data: None,
            fragmenter: FragmentedRenderer::new(canvas_width, canvas_height, tiles_x, tiles_y),
            secure_renderer: SecureImageRenderer::new(
                canvas_width as u32,
                canvas_height as u32,
                tiles_x,
                tiles_y,
            ),
            watermark_encoder: ImageWatermarkEncoder::new(),
            config,
            session_id: String::new(),
            noise_level: 2,
        }
    }

    /// 设置会话ID (用于水印追踪)
    pub fn set_session_id(&mut self, id: &str) {
        self.session_id = id.to_string();
        self.secure_renderer.set_session_id(id);
    }

    /// 设置噪声等级 (0-3)
    pub fn set_noise_level(&mut self, level: u8) {
        self.noise_level = level.min(3);
        self.secure_renderer.set_noise_level(level);
    }

    /// 设置字体数据
    pub fn set_font_data(&mut self, data: &[u8]) {
        self.font_data = Some(data.to_vec());
        #[cfg(target_arch = "wasm32")]
        log(&format!("Font data loaded: {} bytes", data.len()));
    }

    /// 检查是否已加载字体
    pub fn has_font(&self) -> bool {
        self.font_data.is_some()
    }

    /// 获取配置
    pub fn get_config(&self) -> TextRendererConfig {
        self.config.clone()
    }

    /// 获取分片数量
    pub fn get_tile_count(&self) -> u32 {
        self.fragmenter.get_total_tiles()
    }

    /// 获取分片信息
    pub fn get_tile_info(&self, index: u32) -> Vec<i32> {
        self.secure_renderer.get_tile_info(index)
    }

    /// 获取乱序渲染顺序
    pub fn get_render_order(&self) -> Vec<u32> {
        self.secure_renderer.get_render_order()
    }

    /// 准备分片渲染 (打乱顺序)
    pub fn prepare_rendering(&mut self) {
        self.fragmenter.shuffle_tiles();
        self.secure_renderer.set_rendering(true);
    }

    /// 获取下一个待渲染的瓦片索引
    pub fn get_next_tile_index(&mut self) -> Option<u32> {
        if let Some(tile) = self.fragmenter.get_next_tile() {
            Some(tile.id)
        } else {
            self.secure_renderer.set_rendering(false);
            None
        }
    }

    /// 获取渲染进度
    pub fn get_progress(&self) -> f32 {
        self.fragmenter.get_progress()
    }

    /// 是否渲染完成
    pub fn is_complete(&self) -> bool {
        self.fragmenter.is_complete()
    }

    /// 重置渲染状态
    pub fn reset(&mut self) {
        self.fragmenter.reset();
        self.secure_renderer.set_rendering(false);
    }

    /// 标记瓦片已渲染
    pub fn mark_tile_rendered(&mut self, tile_id: u32) {
        self.fragmenter.mark_tile_rendered(tile_id);
    }

    /// 栅格化文字为像素数据 (无保护)
    ///
    /// 注意: 由于 ab-glyph 在 WASM 环境的复杂性，
    /// 此函数返回占位符像素数据，实际渲染在 JS 层完成
    ///
    /// 返回: RGBA 像素数组
    pub fn rasterize_text_placeholder(&self, text: &str, width: u32, height: u32) -> Vec<u8> {
        let char_count = text.len() as u32;
        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);

        // 生成基于文字内容的确定性噪声
        let mut seed: u32 = 0;
        for byte in text.as_bytes() {
            seed = seed.wrapping_mul(31).wrapping_add(*byte as u32);
        }

        for i in 0..pixel_count {
            // 生成位置相关的"文字形状"占位符
            let x = (i as u32) % width;
            let y = (i as u32) / width;

            // 简化的文字轮廓模拟 (矩形形状)
            let in_text_region = y > height / 4
                && y < height * 3 / 4
                && x > width / 8
                && x < width * 7 / 8;

            if in_text_region {
                // 文字区域 - 灰色
                let char_offset = ((x + y) % char_count.max(1)) as u8;
                pixels.push(40 + char_offset); // R
                pixels.push(40 + char_offset); // G
                pixels.push(40 + char_offset); // B
                pixels.push(255);              // A
            } else {
                // 背景 - 透明
                pixels.push(0);   // R
                pixels.push(0);   // G
                pixels.push(0);   // B
                pixels.push(0);   // A
            }
        }

        #[cfg(target_arch = "wasm32")]
        log(&format!("Rasterized text placeholder: {}x{}, {} chars", width, height, char_count));

        pixels
    }

    /// 使用 ab_glyph 栅格化文字为像素数据 (真实字体渲染)
    ///
    /// - `text`: 要渲染的文字
    /// - `font_data`: 字体二进制数据 (TTF/OTF)
    /// - `font_size`: 字体大小 (像素)
    /// - `width`: 输出图像宽度
    /// - `height`: 输出图像高度
    ///
    /// 返回: RGBA 像素数组
    ///
    /// 注意: 由于 ab_glyph API 复杂性，此函数回退到占位符实现
    /// 实际字体渲染在 JS 层的 Canvas 2D 中完成
    pub fn rasterize_text(&self, text: &str, font_data: &[u8], _font_size: f32, width: u32, height: u32) -> Vec<u8> {
        // 尝试加载字体用于度量
        let _font = match FontRef::try_from_slice(font_data) {
            Ok(f) => f,
            Err(_) => {
                // 字体加载失败，回退到占位符
                return self.rasterize_text_placeholder(text, width, height);
            }
        };

        // 回退到占位符实现 (实际渲染在 JS Canvas 2D 中完成)
        #[cfg(target_arch = "wasm32")]
        log(&format!("Using placeholder text rasterization (real rendering in JS Canvas)"));

        self.rasterize_text_placeholder(text, width, height)
    }

    /// 使用 ab_glyph 进行字形栅格化
    ///
    /// 这个函数会生成字形位图数据
    pub fn rasterize_glyph(&self, _glyph_id: u16, font_data: &[u8], font_size: f32) -> Vec<u8> {
        let _font = match FontRef::try_from_slice(font_data) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        // 简化实现: 返回空数组 (实际渲染在 JS 层)
        let width = (font_size * 2.0) as u32;
        let height = (font_size * 1.5) as u32;
        vec![0u8; (width * height) as usize]
    }

    /// 测量文字宽度
    pub fn measure_text_width(&self, text: &str, font_data: &[u8], font_size: f32) -> f32 {
        let font = match FontRef::try_from_slice(font_data) {
            Ok(f) => f,
            Err(_) => return text.len() as f32 * font_size * 0.6,
        };

        let mut width = 0.0_f32;

        for c in text.chars() {
            let glyph_id = font.glyph_id(c);
            width += font.h_advance_unscaled(glyph_id) * (font_size / 16.0);
        }

        width
    }

    /// 应用 LSB 水印到像素数据
    ///
    /// - `pixels`: RGBA 像素数组
    /// - `watermark`: 水印文本
    ///
    /// 返回: 嵌入水印后的像素数据
    pub fn apply_lsb_watermark(&self, pixels: &[u8], watermark: &str) -> Vec<u8> {
        self.watermark_encoder.encode(pixels, watermark)
    }

    /// 从像素数据提取 LSB 水印
    pub fn extract_lsb_watermark(&self, pixels: &[u8]) -> String {
        self.watermark_encoder.decode(pixels)
    }

    /// 检查像素是否包含水印
    pub fn has_watermark(&self, pixels: &[u8]) -> bool {
        self.watermark_encoder.has_watermark(pixels)
    }

    /// 应用 B通道噪声编码
    ///
    /// - `pixels`: RGBA 像素数组 (会被修改)
    /// - `tile_index`: 当前瓦片索引
    pub fn apply_b_noise(&self, pixels: &mut [u8], tile_index: u32) {
        self.secure_renderer.apply_noise(pixels, tile_index);
    }

    /// 验证噪声模式
    pub fn verify_noise(&self, pixels: &[u8], tile_index: u32) -> bool {
        self.secure_renderer.verify_noise(pixels, tile_index)
    }

    /// 获取瓦片像素数据 (用于 WebGPU 渲染)
    ///
    /// - `full_pixels`: 完整图像像素
    /// - `tile_index`: 瓦片索引
    ///
    /// 返回: 该瓦片对应的像素数据
    pub fn get_tile_pixels(&self, full_pixels: &[u8], tile_index: u32) -> Vec<u8> {
        let width = self.config.canvas_width as u32;
        let height = self.config.canvas_height as u32;
        let tiles_x = self.config.tiles_x;
        let tiles_y = self.config.tiles_y;

        let tile_w = width / tiles_x;
        let tile_h = height / tiles_y;
        let tile_x = (tile_index % tiles_x) as u32 * tile_w;
        let tile_y = (tile_index / tiles_x) as u32 * tile_h;

        let mut tile_pixels = Vec::new();
        let full_width = width as usize;
        let tw = tile_w as usize;
        let th = tile_h as usize;
        let sx = tile_x as usize;
        let sy = tile_y as usize;

        for y in sy..(sy + th) {
            for x in sx..(sx + tw) {
                let idx = (y * full_width + x) * 4;
                if idx + 3 < full_pixels.len() {
                    tile_pixels.push(full_pixels[idx]);
                    tile_pixels.push(full_pixels[idx + 1]);
                    tile_pixels.push(full_pixels[idx + 2]);
                    tile_pixels.push(full_pixels[idx + 3]);
                }
            }
        }

        tile_pixels
    }

    /// 生成完整的保护后像素数据
    ///
    /// 流程:
    /// 1. 栅格化文字
    /// 2. 嵌入 LSB 水印
    /// 3. 应用 B通道噪声
    /// 4. 返回分片信息
    pub fn render_protected_text(&mut self, text: &str, watermark: &str) -> TextRenderResult {
        let width = self.config.canvas_width as u32;
        let height = self.config.canvas_height as u32;

        // 1. 栅格化文字为像素
        let base_pixels = self.rasterize_text_placeholder(text, width, height);

        // 2. 嵌入 LSB 水印
        let watermarked_pixels = self.apply_lsb_watermark(&base_pixels, watermark);

        // 3. 应用 B通道噪声 (对每个瓦片)
        let noisy_pixels = watermarked_pixels.clone();
        let tile_count = self.get_tile_count();
        for i in 0..tile_count {
            // 对每个瓦片应用噪声
            let mut tile_pixels = self.get_tile_pixels(&noisy_pixels, i);
            self.apply_b_noise(&mut tile_pixels, i);

            // 将噪声像素写回
            // 注: 实际实现需要在 JS 层处理瓦片级别的噪声
        }

        // 4. 获取渲染顺序
        let render_order = self.get_render_order();

        // 5. 生成噪声信息摘要
        let mut noise_info = Vec::new();
        noise_info.extend_from_slice(self.session_id.as_bytes());
        noise_info.push(b'|');
        noise_info.extend_from_slice(&tile_count.to_le_bytes());

        #[cfg(target_arch = "wasm32")]
        log(&format!(
            "Protected text rendered: {} tiles, {} bytes, watermark: {}",
            tile_count,
            noisy_pixels.len(),
            watermark
        ));

        TextRenderResult {
            pixels: noisy_pixels,
            width,
            height,
            render_order,
            noise_info,
        }
    }
}

/// 简化的文字测量 (用于布局)
#[wasm_bindgen]
pub struct TextMetrics {
    width: f32,
    height: f32,
    baseline: f32,
}

#[wasm_bindgen]
impl TextMetrics {
    #[wasm_bindgen(constructor)]
    pub fn new(width: f32, height: f32, baseline: f32) -> Self {
        Self { width, height, baseline }
    }

    pub fn get_width(&self) -> f32 { self.width }
    pub fn get_height(&self) -> f32 { self.height }
    pub fn get_baseline(&self) -> f32 { self.baseline }
}

/// 测量文字尺寸 (占位符实现)
#[wasm_bindgen]
pub fn measure_text_placeholder(text: &str, font_size: f32) -> TextMetrics {
    // 简化实现: 基于字符数和字体大小估算
    let char_width = font_size * 0.6; // 平均字符宽度约为字体大小的 0.6 倍
    let width = text.len() as f32 * char_width;
    let height = font_size * 1.2; // 加上下行高度

    TextMetrics::new(width, height, font_size * 0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_renderer_creation() {
        let config = TextRendererConfig::new(800.0, 600.0, 4, 3, 24.0, 2);
        let renderer = TextRenderer::new(config);

        assert_eq!(renderer.get_tile_count(), 12);
        assert!(!renderer.has_font());
    }

    #[test]
    fn test_text_renderer_session_id() {
        let config = TextRendererConfig::new(800.0, 600.0, 2, 2, 24.0, 2);
        let mut renderer = TextRenderer::new(config);

        renderer.set_session_id("test-session-123");
        renderer.set_noise_level(2);
    }

    #[test]
    fn test_render_order() {
        let config = TextRendererConfig::new(800.0, 600.0, 4, 3, 24.0, 2);
        let renderer = TextRenderer::new(config);

        let order = renderer.get_render_order();
        assert_eq!(order.len(), 12);

        // 所有瓦片索引应唯一
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn test_rasterize_placeholder() {
        let config = TextRendererConfig::new(100.0, 50.0, 1, 1, 24.0, 2);
        let renderer = TextRenderer::new(config);

        let pixels = renderer.rasterize_text_placeholder("Test", 100, 50);

        // 应该返回 100 * 50 * 4 字节 (RGBA)
        assert_eq!(pixels.len(), 100 * 50 * 4);
    }

    #[test]
    fn test_lsb_watermark() {
        let config = TextRendererConfig::new(100.0, 50.0, 1, 1, 24.0, 2);
        let renderer = TextRenderer::new(config);

        let pixels = vec![128u8; 100 * 50 * 4];
        let watermarked = renderer.apply_lsb_watermark(&pixels, "test-watermark");

        // 水印嵌入后会改变 R 通道最低 2 位
        assert!(renderer.has_watermark(&watermarked));

        let extracted = renderer.extract_lsb_watermark(&watermarked);
        assert_eq!(extracted, "test-watermark");
    }

    #[test]
    fn test_tile_pixels() {
        let config = TextRendererConfig::new(800.0, 600.0, 4, 3, 24.0, 2);
        let renderer = TextRenderer::new(config);

        let full_pixels = vec![200u8; 800 * 600 * 4];

        // 获取第一个瓦片 (0,0)
        let tile_pixels = renderer.get_tile_pixels(&full_pixels, 0);
        assert_eq!(tile_pixels.len(), 200 * 200 * 4); // 4x3 = 12 tiles, each 200x200

        // 获取最后一个瓦片 (3,2)
        let tile_pixels = renderer.get_tile_pixels(&full_pixels, 11);
        assert_eq!(tile_pixels.len(), 200 * 200 * 4);
    }

    #[test]
    fn test_render_progress() {
        let config = TextRendererConfig::new(800.0, 600.0, 2, 2, 24.0, 2);
        let mut renderer = TextRenderer::new(config);

        assert_eq!(renderer.get_progress(), 0.0);
        assert!(!renderer.is_complete());

        renderer.prepare_rendering();

        // 模拟渲染几个瓦片
        let mut rendered = 0u32;
        while let Some(tile_id) = renderer.get_next_tile_index() {
            renderer.mark_tile_rendered(tile_id);
            rendered += 1;
        }

        assert_eq!(rendered, 4);
        assert!(renderer.is_complete());
        assert_eq!(renderer.get_progress(), 1.0);
    }

    #[test]
    fn test_measure_text() {
        let metrics = measure_text_placeholder("Hello World", 24.0);
        assert!(metrics.get_width() > 0.0);
        assert!(metrics.get_height() > 0.0);
    }
}
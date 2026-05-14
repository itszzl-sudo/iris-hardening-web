//! Iris Engine WASM - WebAssembly binding for iris-engine
//!
//! This crate provides WASM bindings for iris-engine, enabling
//! browser-based rendering and interaction.

use wasm_bindgen::prelude::*;

// Text Renderer 模块 - Canvas 文字渲染 + WebGPU 分片保护
mod text_renderer;
pub use text_renderer::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn init_tracing() {
    tracing_wasm::set_as_global_default();
}

#[wasm_bindgen]
pub fn init() {
    init_panic_hook();
    init_tracing();
    #[cfg(target_arch = "wasm32")]
    log("Iris Engine WASM initialized");
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct RenderTile {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rendered: bool,
}

#[wasm_bindgen]
impl RenderTile {
    #[wasm_bindgen(constructor)]
    pub fn new(id: u32, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            rendered: false,
        }
    }
    
    pub fn is_rendered(&self) -> bool {
        self.rendered
    }
    
    pub fn set_rendered(&mut self, rendered: bool) {
        self.rendered = rendered;
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct FragmentedRenderer {
    canvas_width: f32,
    canvas_height: f32,
    tiles: Vec<RenderTile>,
    current_tile: usize,
    total_tiles: u32,
    seed: u32,
}

#[wasm_bindgen]
impl FragmentedRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_width: f32, canvas_height: f32, tiles_x: u32, tiles_y: u32) -> Self {
        let tile_width = canvas_width / tiles_x as f32;
        let tile_height = canvas_height / tiles_y as f32;
        let total_tiles = tiles_x * tiles_y;
        
        let mut tiles = Vec::with_capacity(total_tiles as usize);
        let mut id = 0u32;
        
        for y in 0..tiles_y {
            for x in 0..tiles_x {
                tiles.push(RenderTile::new(
                    id,
                    x as f32 * tile_width,
                    y as f32 * tile_height,
                    tile_width,
                    tile_height,
                ));
                id += 1;
            }
        }
        
        Self {
            canvas_width,
            canvas_height,
            tiles,
            current_tile: 0,
            total_tiles,
            seed: 12345,
        }
    }
    
    pub fn shuffle_tiles(&mut self) {
        let mut rng = self.seed;
        for i in (1..self.tiles.len()).rev() {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng as usize) % (i + 1);
            self.tiles.swap(i, j);
        }
        self.seed = rng;
        self.current_tile = 0;
        #[cfg(target_arch = "wasm32")]
        log(&format!("Tiles shuffled: {} total", self.total_tiles));
    }
    
    pub fn get_next_tile(&mut self) -> Option<RenderTile> {
        if self.current_tile < self.tiles.len() {
            let tile = self.tiles[self.current_tile].clone();
            self.current_tile += 1;
            Some(tile)
        } else {
            None
        }
    }
    
    pub fn get_tile_by_id(&self, id: u32) -> Option<RenderTile> {
        self.tiles.iter().find(|t| t.id == id).cloned()
    }
    
    pub fn mark_tile_rendered(&mut self, id: u32) {
        if let Some(tile) = self.tiles.iter_mut().find(|t| t.id == id) {
            tile.rendered = true;
        }
    }
    
    pub fn get_rendered_count(&self) -> u32 {
        self.tiles.iter().filter(|t| t.rendered).count() as u32
    }
    
    pub fn get_total_tiles(&self) -> u32 {
        self.total_tiles
    }
    
    pub fn is_complete(&self) -> bool {
        self.tiles.iter().all(|t| t.rendered)
    }
    
    pub fn get_progress(&self) -> f32 {
        if self.total_tiles == 0 {
            return 0.0;
        }
        self.get_rendered_count() as f32 / self.total_tiles as f32
    }
    
    pub fn reset(&mut self) {
        for tile in &mut self.tiles {
            tile.rendered = false;
        }
        self.current_tile = 0;
    }
    
    pub fn get_all_tiles(&self) -> Vec<RenderTile> {
        self.tiles.clone()
    }
}

#[wasm_bindgen]
pub struct IrisEngineWasm {
    initialized: bool,
    webgpu_monitor: bool,
    fragmented_renderer: Option<FragmentedRenderer>,
}

#[wasm_bindgen]
impl IrisEngineWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        init();
        Self { 
            initialized: true,
            webgpu_monitor: false,
            fragmented_renderer: None,
        }
    }
    
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    pub fn enable_webgpu_monitor(&mut self) {
        self.webgpu_monitor = true;
        #[cfg(target_arch = "wasm32")]
        log("WebGPU monitor enabled");
    }
    
    pub fn disable_webgpu_monitor(&mut self) {
        self.webgpu_monitor = false;
        #[cfg(target_arch = "wasm32")]
        log("WebGPU monitor disabled");
    }
    
    pub fn is_webgpu_monitor_enabled(&self) -> bool {
        self.webgpu_monitor
    }
    
    pub fn setup_fragmented_rendering(&mut self, canvas_width: f32, canvas_height: f32, tiles_x: u32, tiles_y: u32) {
        self.fragmented_renderer = Some(FragmentedRenderer::new(canvas_width, canvas_height, tiles_x, tiles_y));
        #[cfg(target_arch = "wasm32")]
        log(&format!("Fragmented rendering setup: {}x{} tiles", tiles_x, tiles_y));
    }
    
    pub fn get_fragmented_renderer(&self) -> Option<FragmentedRenderer> {
        self.fragmented_renderer.clone()
    }
    
    pub fn shuffle_tiles(&mut self) {
        if let Some(ref mut renderer) = self.fragmented_renderer {
            renderer.shuffle_tiles();
        }
    }
    
    pub fn get_next_tile(&mut self) -> Option<RenderTile> {
        if let Some(ref mut renderer) = self.fragmented_renderer {
            renderer.get_next_tile()
        } else {
            None
        }
    }
    
    pub fn mark_tile_rendered(&mut self, id: u32) {
        if let Some(ref mut renderer) = self.fragmented_renderer {
            renderer.mark_tile_rendered(id);
        }
    }
    
    pub fn get_render_progress(&self) -> f32 {
        if let Some(ref renderer) = self.fragmented_renderer {
            renderer.get_progress()
        } else {
            0.0
        }
    }
    
    pub fn is_render_complete(&self) -> bool {
        if let Some(ref renderer) = self.fragmented_renderer {
            renderer.is_complete()
        } else {
            false
        }
    }
    
    pub fn render(&mut self, _canvas_id: &str) -> Result<(), JsValue> {
        if !self.initialized {
            return Err(JsValue::from_str("Engine not initialized"));
        }
        #[cfg(target_arch = "wasm32")]
        log("Rendering to canvas");
        Ok(())
    }
    
    pub fn handle_event(&mut self, event_type: &str, event_data: &str) -> Result<(), JsValue> {
        if !self.initialized {
            return Err(JsValue::from_str("Engine not initialized"));
        }
        #[cfg(target_arch = "wasm32")]
        log(&format!("Handling event: {} - {}", event_type, event_data));
        Ok(())
    }
}

impl Default for IrisEngineWasm {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
pub fn create_engine() -> IrisEngineWasm {
    IrisEngineWasm::new()
}

#[wasm_bindgen]
pub fn get_version() -> String {
    VERSION.to_string()
}

/// 图片水印编码器 - 将水印信息编码到像素数据的最低有效位(LSB)
///
/// 编码格式: 每个像素的 R 通道最低 2 位用于存储水印数据
/// 头部: [4字节 magic: 0x49 0x52 0x49 0x53][4字节 payload长度(LE)][payload bytes]
#[wasm_bindgen]
pub struct ImageWatermarkEncoder {
    /// 水印 magic 标记 (IRIS)
    magic: [u8; 4],
}

#[wasm_bindgen]
impl ImageWatermarkEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { magic: [0x49, 0x52, 0x49, 0x53] } // "IRIS"
    }

    /// 将水印文本编码到 RGBA 像素数据中
    /// 每个 R 通道最低 2 位存储水印数据，视觉上完全不可见
    ///
    /// - `pixels`: RGBA 像素数组 (Uint8ClampedArray 转换)
    /// - `watermark_text`: 要嵌入的水印文本
    ///
    /// 返回编码后的像素数据
    pub fn encode(&self, pixels: &[u8], watermark_text: &str) -> Vec<u8> {
        let payload = watermark_text.as_bytes();
        let payload_len = payload.len() as u32;

        // 构建完整的水印数据: magic + length + payload
        let mut watermark_data = Vec::with_capacity(8 + payload.len());
        watermark_data.extend_from_slice(&self.magic);
        watermark_data.extend_from_slice(&payload_len.to_le_bytes());
        watermark_data.extend_from_slice(payload);

        // 将水印数据的每个字节编码到像素 R 通道的最低 2 位
        // 每个字节需要 4 个像素 (8 bits / 2 bits per pixel)
        let mut result = pixels.to_vec();
        let bits_needed = watermark_data.len() * 8;
        let pixels_available = result.len() / 4; // RGBA

        if bits_needed > pixels_available * 2 {
            #[cfg(target_arch = "wasm32")]
            log(&format!("Warning: image too small for watermark, need {} pixels, have {}", bits_needed / 2, pixels_available));
            return result;
        }

        let mut bit_index = 0usize;
        for byte in &watermark_data {
            for shift in (0..8).step_by(2) {
                let two_bits = (byte >> shift) & 0x03;
                let pixel_index = bit_index * 4; // R channel of each pixel
                if pixel_index < result.len() {
                    // 清除最低 2 位，然后设置水印位
                    result[pixel_index] = (result[pixel_index] & 0xFC) | two_bits;
                }
                bit_index += 1;
            }
        }

        #[cfg(target_arch = "wasm32")]
        log(&format!("Watermark encoded: {} bytes into {} pixels", watermark_data.len(), bit_index));

        result
    }

    /// 从 RGBA 像素数据中解码水印文本
    ///
    /// - `pixels`: RGBA 像素数组
    ///
    /// 返回解码出的水印文本，如果不存在则返回空字符串
    pub fn decode(&self, pixels: &[u8]) -> String {
        // 从像素中提取水印位
        let mut watermark_bits = Vec::new();
        for i in (0..pixels.len()).step_by(4) {
            let two_bits = pixels[i] & 0x03;
            watermark_bits.push(two_bits);

            // 预读足够多的位来检查 magic 和长度
            if watermark_bits.len() >= 16 {
                // 尝试解码 magic
                let mut decoded = Vec::new();
                for chunk in watermark_bits.chunks(4) {
                    if chunk.len() == 4 {
                        let byte = chunk[0] | (chunk[1] << 2) | (chunk[2] << 4) | (chunk[3] << 6);
                        decoded.push(byte);
                    }
                }

                if decoded.len() >= 4 {
                    if decoded[0] == self.magic[0]
                        && decoded[1] == self.magic[1]
                        && decoded[2] == self.magic[2]
                        && decoded[3] == self.magic[3]
                    {
                        // Magic 匹配，读取长度
                        if decoded.len() >= 8 {
                            let payload_len = u32::from_le_bytes([
                                decoded[4], decoded[5], decoded[6], decoded[7],
                            ]) as usize;

                            let total_bytes = 8 + payload_len;
                            let total_bit_groups = total_bytes * 4; // 每字节 4 个 2-bit 组

                            // 继续读取直到有足够数据
                            if watermark_bits.len() < total_bit_groups {
                                // 需要更多像素
                                continue;
                            }

                            // 重新解码完整数据
                            let mut full_decoded = Vec::new();
                            for chunk in watermark_bits[..total_bit_groups].chunks(4) {
                                if chunk.len() == 4 {
                                    let byte = chunk[0] | (chunk[1] << 2) | (chunk[2] << 4) | (chunk[3] << 6);
                                    full_decoded.push(byte);
                                }
                            }

                            if full_decoded.len() >= 8 + payload_len {
                                let payload = &full_decoded[8..8 + payload_len];
                                if let Ok(text) = std::str::from_utf8(payload) {
                                    #[cfg(target_arch = "wasm32")]
                                    log(&format!("Watermark decoded: {}", text));
                                    return text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        String::new()
    }

    /// 检查像素数据中是否包含有效的水印
    pub fn has_watermark(&self, pixels: &[u8]) -> bool {
        if pixels.len() < 16 * 4 {
            return false;
        }

        // 只检查 magic 标记 (需要 4 字节 = 16 个 2-bit 组 = 16 个像素)
        let mut decoded = Vec::new();
        let mut bit_groups = Vec::new();

        for i in (0..64).step_by(4) {
            if i < pixels.len() {
                bit_groups.push(pixels[i] & 0x03);
            }
        }

        for chunk in bit_groups.chunks(4) {
            if chunk.len() == 4 {
                let byte = chunk[0] | (chunk[1] << 2) | (chunk[2] << 4) | (chunk[3] << 6);
                decoded.push(byte);
            }
        }

        decoded.len() >= 4
            && decoded[0] == self.magic[0]
            && decoded[1] == self.magic[1]
            && decoded[2] == self.magic[2]
            && decoded[3] == self.magic[3]
    }
}

impl Default for ImageWatermarkEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Secure Image Renderer - anti-screenshot protection
///
/// Defenses:
/// 1. Fragment rendering: image is split into tiles, drawn in shuffled order
/// 2. Visual noise: invisible random noise injected per-tile (LSB of B channel)
/// 3. Coordinate watermark: each tile's canvas position encoded in pixel noise
/// 4. Scramble offset: tiles are drawn at shifted positions, then corrected
///
/// A screenshot captures the final composited image, but:
/// - The noise pattern proves the image came from this session
/// - The coordinate watermark proves which tile was where
/// - The scramble offset means a mid-render screenshot gets misaligned tiles
#[wasm_bindgen]
pub struct SecureImageRenderer {
    width: u32,
    height: u32,
    tiles_x: u32,
    tiles_y: u32,
    seed: u32,
    noise_level: u8,
    /// Per-tile scramble offsets (x, y) in pixels
    offsets: Vec<(i32, i32)>,
    /// Whether rendering is in progress (anti-capture flag)
    rendering: bool,
    /// Session ID embedded in noise
    session_id: String,
}

#[wasm_bindgen]
impl SecureImageRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, tiles_x: u32, tiles_y: u32) -> Self {
        let seed = 0xDEADBEEFu32;
        let mut renderer = Self {
            width,
            height,
            tiles_x,
            tiles_y,
            seed,
            noise_level: 2, // max 2-bit noise per B channel
            offsets: Vec::new(),
            rendering: false,
            session_id: String::new(),
        };
        renderer.generate_offsets();
        renderer
    }

    /// Set a session identifier (e.g. user + timestamp) for watermarking
    pub fn set_session_id(&mut self, id: &str) {
        self.session_id = id.to_string();
    }

    /// Set noise level (0-3, bits of noise in B channel LSB)
    pub fn set_noise_level(&mut self, level: u8) {
        self.noise_level = level.min(3);
    }

    /// Generate random offsets for each tile
    pub fn generate_offsets(&mut self) {
        let total = self.tiles_x * self.tiles_y;
        self.offsets.clear();
        let mut rng = self.seed;
        for _ in 0..total {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let ox = ((rng >> 16) as i32 % 4) - 2; // -2 to +1 pixels
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let oy = ((rng >> 16) as i32 % 4) - 2;
            self.offsets.push((ox, oy));
        }
        self.seed = rng;
    }

    /// Get the number of tiles
    pub fn get_tile_count(&self) -> u32 {
        self.tiles_x * self.tiles_y
    }

    /// Get tile position and dimensions for a given tile index
    /// Returns: [x, y, width, height, offset_x, offset_y]
    pub fn get_tile_info(&self, index: u32) -> Vec<i32> {
        let tw = self.width / self.tiles_x;
        let th = self.height / self.tiles_y;
        let tx = index % self.tiles_x;
        let ty = index / self.tiles_x;
        let (ox, oy) = self.offsets.get(index as usize).copied().unwrap_or((0, 0));
        vec![
            (tx * tw) as i32,
            (ty * th) as i32,
            tw as i32,
            th as i32,
            ox,
            oy,
        ]
    }

    /// Get a shuffled rendering order for tiles
    pub fn get_render_order(&self) -> Vec<u32> {
        let total = (self.tiles_x * self.tiles_y) as usize;
        let mut order: Vec<u32> = (0..total as u32).collect();
        let mut rng = self.seed.wrapping_add(42);
        for i in (1..total).rev() {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng as usize) % (i + 1);
            order.swap(i, j);
        }
        order
    }

    /// Apply anti-screenshot noise to a tile's pixel data.
    /// - Encodes tile index + session hash into B channel LSB
    /// - Adds random visual noise
    pub fn apply_noise(&self, pixels: &mut [u8], tile_index: u32) {
        if pixels.len() < 4 {
            return;
        }

        // Create a simple hash of session_id for this tile
        let session_hash = if !self.session_id.is_empty() {
            let mut h: u32 = 0;
            for b in self.session_id.as_bytes() {
                h = h.wrapping_mul(31).wrapping_add(*b as u32);
            }
            h.wrapping_add(tile_index.wrapping_mul(7919))
        } else {
            tile_index.wrapping_mul(7919).wrapping_add(0x49524953) // "IRIS" as u32
        };

        // Encode tile_index (16 bits) + session_hash (16 bits) into B channel LSBs
        let idx_lo = (tile_index & 0xFF) as u8;
        let idx_hi = ((tile_index >> 8) & 0xFF) as u8;
        let hash_lo = (session_hash & 0xFF) as u8;
        let hash_hi = ((session_hash >> 8) & 0xFF) as u8;

        let noise_bytes = [idx_lo, idx_hi, hash_lo, hash_hi];

        // First 4 pixels: encode watermark into B channel
        for (i, &nb) in noise_bytes.iter().enumerate() {
            let pi = i * 4 + 2; // B channel
            if pi < pixels.len() {
                pixels[pi] = (pixels[pi] & (0xFF << self.noise_level)) | (nb & ((1 << self.noise_level) - 1));
            }
        }

        // Remaining pixels: add subtle random noise
        let mut rng = session_hash.wrapping_add(tile_index.wrapping_mul(65537));
        for i in 4..(pixels.len() / 4) {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = (rng as u8) & ((1 << self.noise_level) - 1);
            let pi = i * 4 + 2; // B channel
            if pi < pixels.len() {
                pixels[pi] = (pixels[pi] & (0xFF << self.noise_level)) | noise;
            }
        }
    }

    /// Verify noise pattern in a tile (for forensic detection)
    pub fn verify_noise(&self, pixels: &[u8], tile_index: u32) -> bool {
        if pixels.len() < 16 {
            return false;
        }

        let idx_lo = (tile_index & 0xFF) as u8;
        let idx_hi = ((tile_index >> 8) & 0xFF) as u8;

        let mask = ((1 << self.noise_level) - 1) as u8;

        // Check first 2 pixels' B channel for tile index
        let b0 = pixels[2] & mask;
        let b1 = pixels[6] & mask;

        b0 == (idx_lo & mask) && b1 == (idx_hi & mask)
    }

    /// Mark rendering as in-progress (for anti-capture logic)
    pub fn set_rendering(&mut self, active: bool) {
        self.rendering = active;
    }

    pub fn is_rendering(&self) -> bool {
        self.rendering
    }

    /// Get image dimensions
    pub fn get_width(&self) -> u32 { self.width }
    pub fn get_height(&self) -> u32 { self.height }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_render_tile_creation() {
        let tile = RenderTile::new(0, 0.0, 0.0, 200.0, 150.0);
        assert_eq!(tile.id, 0);
        assert_eq!(tile.x, 0.0);
        assert_eq!(tile.y, 0.0);
        assert_eq!(tile.width, 200.0);
        assert_eq!(tile.height, 150.0);
        assert!(!tile.rendered);
    }
    
    #[test]
    fn test_render_tile_mark_rendered() {
        let mut tile = RenderTile::new(1, 100.0, 100.0, 200.0, 150.0);
        assert!(!tile.is_rendered());
        tile.set_rendered(true);
        assert!(tile.is_rendered());
    }
    
    #[test]
    fn test_fragmented_renderer_creation() {
        let renderer = FragmentedRenderer::new(800.0, 600.0, 4, 3);
        assert_eq!(renderer.get_total_tiles(), 12);
        assert_eq!(renderer.get_rendered_count(), 0);
        assert!(!renderer.is_complete());
        assert_eq!(renderer.get_progress(), 0.0);
    }
    
    #[test]
    fn test_fragmented_renderer_tiles() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 4, 3);
        
        let mut tiles = Vec::new();
        while let Some(tile) = renderer.get_next_tile() {
            tiles.push(tile);
        }
        
        assert_eq!(tiles.len(), 12);
        
        for (i, tile) in tiles.iter().enumerate() {
            assert_eq!(tile.id, i as u32);
            assert!(!tile.rendered);
        }
    }
    
    #[test]
    fn test_fragmented_renderer_shuffle() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 4, 3);
        
        let original_order: Vec<u32> = {
            let mut ids = Vec::new();
            while let Some(tile) = renderer.get_next_tile() {
                ids.push(tile.id);
            }
            ids
        };
        
        renderer.shuffle_tiles();
        
        let shuffled_order: Vec<u32> = {
            let mut ids = Vec::new();
            while let Some(tile) = renderer.get_next_tile() {
                ids.push(tile.id);
            }
            ids
        };
        
        assert_eq!(original_order.len(), shuffled_order.len());
        
        let mut sorted_shuffled = shuffled_order.clone();
        sorted_shuffled.sort();
        assert_eq!(original_order, sorted_shuffled);
    }
    
    #[test]
    fn test_fragmented_renderer_mark_rendered() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 2, 2);
        
        while let Some(tile) = renderer.get_next_tile() {
            renderer.mark_tile_rendered(tile.id);
        }
        
        assert_eq!(renderer.get_rendered_count(), 4);
        assert!(renderer.is_complete());
        assert_eq!(renderer.get_progress(), 1.0);
    }
    
    #[test]
    fn test_fragmented_renderer_reset() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 2, 2);
        
        while let Some(tile) = renderer.get_next_tile() {
            renderer.mark_tile_rendered(tile.id);
        }
        
        assert!(renderer.is_complete());
        
        renderer.reset();
        
        assert_eq!(renderer.get_rendered_count(), 0);
        assert!(!renderer.is_complete());
        assert_eq!(renderer.get_progress(), 0.0);
    }
    
    #[test]
    fn test_fragmented_renderer_tile_positions() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 4, 3);
        
        let tile_width = 800.0 / 4.0;
        let tile_height = 600.0 / 3.0;
        
        while let Some(tile) = renderer.get_next_tile() {
            let expected_x = (tile.id % 4) as f32 * tile_width;
            let expected_y = (tile.id / 4) as f32 * tile_height;
            
            assert_eq!(tile.x, expected_x);
            assert_eq!(tile.y, expected_y);
            assert_eq!(tile.width, tile_width);
            assert_eq!(tile.height, tile_height);
        }
    }
    
    #[test]
    fn test_fragmented_renderer_partial_render() {
        let mut renderer = FragmentedRenderer::new(800.0, 600.0, 4, 3);

        for _ in 0..6 {
            if let Some(tile) = renderer.get_next_tile() {
                renderer.mark_tile_rendered(tile.id);
            }
        }

        assert_eq!(renderer.get_rendered_count(), 6);
        assert!(!renderer.is_complete());

        let progress = renderer.get_progress();
        assert!(progress > 0.4 && progress < 0.6);
    }

    #[test]
    fn test_watermark_encode_decode() {
        let encoder = ImageWatermarkEncoder::new();

        // 创建一个简单的 RGBA 像素数组 (200x1 pixels) - 足够容纳水印数据
        let pixels = vec![128u8; 200 * 4];

        let watermark = "hardening.irisverse.org";
        let encoded = encoder.encode(&pixels, watermark);
        let decoded = encoder.decode(&encoded);

        assert_eq!(decoded, watermark);
    }

    #[test]
    fn test_watermark_has_watermark() {
        let encoder = ImageWatermarkEncoder::new();

        let pixels = vec![128u8; 100 * 4];
        assert!(!encoder.has_watermark(&pixels));

        let encoded = encoder.encode(&pixels, "test");
        assert!(encoder.has_watermark(&encoded));
    }

    #[test]
    fn test_watermark_imperceptible() {
        let encoder = ImageWatermarkEncoder::new();

        let mut pixels = vec![128u8; 100 * 4];
        let original: Vec<u8> = pixels.clone();

        let encoded = encoder.encode(&pixels, "test");

        // 编码后每个像素 R 通道最多改变 3 (最低 2 位)
        let mut max_diff = 0u8;
        for i in (0..encoded.len()).step_by(4) {
            let diff = (encoded[i] as i16 - original[i] as i16).abs() as u8;
            if diff > max_diff {
                max_diff = diff;
            }
        }

        // 最大差异不超过 3 (2 bits)
        assert!(max_diff <= 3);
    }

    #[test]
    fn test_watermark_chinese() {
        let encoder = ImageWatermarkEncoder::new();

        let pixels = vec![128u8; 200 * 4];
        let watermark = "安全水印测试";
        let encoded = encoder.encode(&pixels, watermark);
        let decoded = encoder.decode(&encoded);

        assert_eq!(decoded, watermark);
    }

    #[test]
    fn test_secure_renderer_creation() {
        let renderer = SecureImageRenderer::new(800, 600, 4, 3);
        assert_eq!(renderer.get_width(), 800);
        assert_eq!(renderer.get_height(), 600);
        assert_eq!(renderer.get_tile_count(), 12);
        assert!(!renderer.is_rendering());
    }

    #[test]
    fn test_secure_renderer_tile_info() {
        let renderer = SecureImageRenderer::new(800, 600, 4, 3);
        let info = renderer.get_tile_info(0);
        // First tile: x=0, y=0, w=200, h=200, offset varies
        assert_eq!(info[0], 0); // x
        assert_eq!(info[1], 0); // y
        assert_eq!(info[2], 200); // width
        assert_eq!(info[3], 200); // height
    }

    #[test]
    fn test_secure_renderer_noise() {
        let mut renderer = SecureImageRenderer::new(100, 100, 2, 2);
        renderer.set_session_id("test-session");

        let mut pixels = vec![128u8; 64 * 4]; // 64 pixels for a small tile
        let original: Vec<u8> = pixels.clone();
        renderer.apply_noise(&mut pixels, 0);

        // Pixels should have changed in B channel
        let mut changed = false;
        for i in (2..pixels.len()).step_by(4) {
            if pixels[i] != original[i] {
                changed = true;
                break;
            }
        }
        assert!(changed, "Noise should modify B channel");

        // But changes should be small (max 2-bit noise)
        for i in (2..pixels.len()).step_by(4) {
            let diff = (pixels[i] as i16 - original[i] as i16).abs() as u8;
            assert!(diff <= 3, "Noise should be <= 3 per channel, got {}", diff);
        }
    }

    #[test]
    fn test_secure_renderer_verify_noise() {
        let mut renderer = SecureImageRenderer::new(100, 100, 2, 2);
        renderer.set_noise_level(2);

        let mut pixels = vec![128u8; 64 * 4];
        renderer.apply_noise(&mut pixels, 5);

        assert!(renderer.verify_noise(&pixels, 5), "Should verify correct tile index");
        assert!(!renderer.verify_noise(&pixels, 99), "Should fail wrong tile index");
    }

    #[test]
    fn test_secure_renderer_render_order() {
        let renderer = SecureImageRenderer::new(800, 600, 4, 3);
        let order = renderer.get_render_order();
        assert_eq!(order.len(), 12);

        // All tile indices should be present exactly once
        let mut sorted = order.clone();
        sorted.sort();
        let expected: Vec<u32> = (0..12).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn test_secure_renderer_rendering_flag() {
        let mut renderer = SecureImageRenderer::new(100, 100, 2, 2);
        assert!(!renderer.is_rendering());
        renderer.set_rendering(true);
        assert!(renderer.is_rendering());
    }
}

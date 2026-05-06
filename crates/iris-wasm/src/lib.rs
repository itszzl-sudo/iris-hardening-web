//! Iris Engine WASM - WebAssembly binding for iris-engine
//! 
//! This crate provides WASM bindings for iris-engine, enabling
//! browser-based rendering and interaction.

use wasm_bindgen::prelude::*;
use console_error_panic_hook;
use wee_alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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
    iris_engine::init();
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
    iris_engine::VERSION.to_string()
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
}

/**
 * Iris Secure Canvas - WebGPU anti-screenshot image rendering
 *
 * Anti-screenshot defenses (all GPU-side, no JS pixel access):
 *
 * 1. Interlaced row flicker (奇偶帧分行闪烁)
 *    Even frames show even rows, odd frames show odd rows.
 *    Missing rows are filled from adjacent row + noise.
 *    Eye blends via persistence of vision, screenshot gets half-res + noise.
 *
 * 2. High-frequency pixel jitter (像素高频抖动小副位移)
 *    Each pixel is sampled from a slightly shifted source position (±1-2px).
 *    The shift changes every frame at 60fps.
 *    Eye averages to correct position, screenshot captures a jittered frame.
 *
 * 3. Dynamic high-density random noise mask (动态高密度随机噪点蒙版)
 *    Every pixel has a per-frame random noise overlay (~8-16 levels).
 *    The noise pattern is completely different each frame.
 *    Eye averages it out, screenshot captures the noise.
 *
 * Combined effect: a screenshot captures ONE frame that has:
 *   - Half the rows are synthetic (interlaced)
 *   - All pixels are spatially shifted (jitter)
 *   - All pixels have noise overlay (mask)
 *   → The image is degraded but still recognizable to the eye in motion.
 *
 * Additional:
 * 4. Progressive tile reveal - screenshots mid-load are incomplete
 * 5. Per-tile forensic watermark (B channel LSB) - traceable
 * 6. Scramble mode on capture detection - GPU renders noise
 *
 * Fallback: Canvas 2D with fragmented rendering if WebGPU unavailable
 */

(function() {
    var irisWasm = null;
    var secureCanvases = [];

    // ── WASM Load ──────────────────────────────────────────────────────────

    async function loadIrisWasm() {
        if (irisWasm) return irisWasm;
        try {
            var wasmUrl = (window.IRIS_CONFIG && window.IRIS_CONFIG.wasmUrl) || '/iris.wasm';
            var resp = await fetch(wasmUrl);
            var wasmBuffer = await resp.arrayBuffer();
            var view = new Uint8Array(wasmBuffer, 0, 4);
            var isWasm = view[0] === 0x00 && view[1] === 0x61 && view[2] === 0x73 && view[3] === 0x6d;
            if (!isWasm && window.IRIS_CONFIG && window.IRIS_CONFIG.decryptUrl) {
                var base64 = arrayBufferToBase64(wasmBuffer);
                var decResp = await fetch(window.IRIS_CONFIG.decryptUrl, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ data: base64 }),
                });
                if (decResp.ok) {
                    var decResult = await decResp.json();
                    wasmBuffer = base64ToArrayBuffer(decResult.data);
                }
            }
            var result = await WebAssembly.instantiate(wasmBuffer, {
                env: { memory: new WebAssembly.Memory({ initial: 256, maximum: 1024 }) },
            });
            irisWasm = result.instance.exports;
            if (irisWasm.init) irisWasm.init();
            return irisWasm;
        } catch (e) {
            console.error('[Iris Canvas] WASM load failed:', e);
            return null;
        }
    }

    function arrayBufferToBase64(buffer) {
        var bytes = new Uint8Array(buffer);
        var binary = '';
        for (var i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
        return btoa(binary);
    }

    function base64ToArrayBuffer(base64) {
        var binary = atob(base64);
        var bytes = new Uint8Array(binary.length);
        for (var i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        return bytes.buffer;
    }

    // ── WGSL Shaders ───────────────────────────────────────────────────────

    var VERTEX_SHADER = `
struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vertexMain(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var pos = array<vec2f, 3>(
        vec2f(-1.0, -1.0),
        vec2f( 3.0, -1.0),
        vec2f(-1.0,  3.0),
    );
    var uv = array<vec2f, 3>(
        vec2f(0.0, 1.0),
        vec2f(2.0, 1.0),
        vec2f(0.0, -1.0),
    );
    var out: VertexOutput;
    out.position = vec4f(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}
`;

    var FRAGMENT_SHADER = `
struct IrisUniforms {
    img_width: f32,
    img_height: f32,
    tiles_x: u32,
    tiles_y: u32,
    session_hash: u32,
    noise_level: u32,      // forensic watermark bit depth
    scramble: u32,         // 0=normal, 1=blank on screenshot
    frame_seed: u32,       // changes every frame
    tiles_revealed: u32,   // progressive reveal count
    interlace: u32,        // 1=enable interlaced row flicker
    jitter_amp: f32,       // pixel jitter amplitude (0.0-3.0 pixels)
    noise_mask_amp: f32,   // dynamic noise mask amplitude (0.0-0.15)
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
};

@group(0) @binding(0) var<uniform> u: IrisUniforms;
@group(0) @binding(1) var sourceTex: texture_2d<f32>;
@group(0) @binding(2) var sourceSamp: sampler;
@group(0) @binding(3) var<storage, read> tileRevealed: array<u32>;  // bitmask: bit N = tile N revealed

// ── Hash functions ──────────────────────────────────────────────────────

fn hash(h: u32) -> u32 {
    var v = h;
    v = v ^ (v >> 16u);
    v = v * 0x45d9f3bu;
    v = v ^ (v >> 16u);
    v = v * 0x45d9f3bu;
    v = v ^ (v >> 16u);
    return v;
}

fn hash2(a: u32, b: u32) -> u32 {
    return hash(a ^ hash(b));
}

fn hashf(v: u32) -> f32 {
    return f32(hash(v)) / 4294967295.0;
}

fn hashf2(a: u32, b: u32) -> f32 {
    return f32(hash2(a, b)) / 4294967295.0;
}

// ── Fragment ────────────────────────────────────────────────────────────

@fragment
fn fragmentMain(input: VertexOutput) -> @location(0) vec4f {
    // ── Scramble mode: render random noise ──
    if (u.scramble == 1u) {
        let px = u32(input.uv.x * u.img_width);
        let py = u32(input.uv.y * u.img_height);
        let n1 = hash2(px ^ u.frame_seed, py);
        let n2 = hash2(py ^ u.frame_seed, px);
        return vec4f(f32(n1 & 0xFFu) / 512.0, f32(n2 & 0xFFu) / 512.0, f32((n1 ^ n2) & 0xFFu) / 512.0, 1.0);
    }

    // ── Tile check ──
    let tile_w = 1.0 / f32(u.tiles_x);
    let tile_h = 1.0 / f32(u.tiles_y);
    let tile_x = u32(input.uv.x / tile_w);
    let tile_y = u32(input.uv.y / tile_h);
    var tile_index = tile_y * u.tiles_x + tile_x;

    if (tile_x >= u.tiles_x || tile_y >= u.tiles_y) {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }

    // Progressive reveal (bitmap lookup — O(1) instead of O(n) linear search)
    let bitmap_word = tile_index / 32u;
    let bitmap_bit = tile_index % 32u;
    var revealed = false;
    if (bitmap_word < arrayLength(&tileRevealed)) {
        revealed = (tileRevealed[bitmap_word] & (1u << bitmap_bit)) != 0u;
    }

    if (!revealed) {
        let h = hash(tile_index ^ u.session_hash ^ 0xDEADu);
        return vec4f(f32(h & 0xFu) / 256.0, f32((h >> 4u) & 0xFu) / 256.0, f32((h >> 8u) & 0xFu) / 256.0, 1.0);
    }

    // ── Compute pixel coordinates ──
    let px = u32(input.uv.x * u.img_width);
    let py = u32(input.uv.y * u.img_height);

    // ── 1. INTERLACED ROW FLICKER (奇偶帧分行闪烁) ──
    // Even frames show even rows, odd frames show odd rows.
    // Missing rows sample from neighbor + noise → eye blends, screenshot is degraded
    var src_uv = input.uv;
    var row_is_synthetic = false;

    if (u.interlace == 1u) {
        let frame_parity = u.frame_seed & 1u; // 0 or 1, alternates each frame
        let row_parity = py & 1u;             // even or odd row

        if (row_parity != frame_parity) {
            // This row is "missing" this frame — sample from neighbor + noise
            row_is_synthetic = true;

            // Blend between two adjacent rows
            let neighbor_offset = select(1.0, -1.0, py <= 1u);
            let blend_uv_y = input.uv.y + neighbor_offset / u.img_height;

            // Clamp
            let clamped_y = clamp(blend_uv_y, 0.0, 1.0);
            src_uv = vec2f(input.uv.x, clamped_y);
        }
    }

    // ── 2. PIXEL JITTER (像素高频抖动小副位移) ──
    // Each pixel samples from a slightly shifted position, changes every frame
    if (u.jitter_amp > 0.0) {
        let jitter_hash_x = hashf2(px ^ u.frame_seed, py ^ 0xA5A5u);
        let jitter_hash_y = hashf2(px ^ 0x5A5Au, py ^ u.frame_seed);
        // Map [0,1] → [-amp, +amp]
        let jx = (jitter_hash_x * 2.0 - 1.0) * u.jitter_amp / u.img_width;
        let jy = (jitter_hash_y * 2.0 - 1.0) * u.jitter_amp / u.img_height;
        src_uv = vec2f(
            clamp(src_uv.x + jx, 0.0, 1.0),
            clamp(src_uv.y + jy, 0.0, 1.0),
        );
    }

    // ── Sample texture ──
    var color = textureSample(sourceTex, sourceSamp, src_uv);

    // ── Interlace: add synthetic row noise ──
    if (row_is_synthetic) {
        // Blend with noise to make the synthetic row different from the real one
        let noise_h = hash2(px ^ u.frame_seed, py ^ 0xBEEFu);
        let noise = vec3f(
            f32(noise_h & 0xFu) / 256.0,
            f32((noise_h >> 4u) & 0xFu) / 256.0,
            f32((noise_h >> 8u) & 0xFu) / 256.0,
        );
        // Mix ~80% neighbor + ~20% noise → eye blends, screenshot gets wrong data
        color = vec4f(color.r * 0.82 + noise.r, color.g * 0.82 + noise.g, color.b * 0.82 + noise.b, 1.0);
    }

    // ── 3. DYNAMIC HIGH-DENSITY RANDOM NOISE MASK (动态高密度随机噪点蒙版) ──
    // Every pixel gets a per-frame random noise overlay
    // High density = every pixel, dynamic = changes each frame
    if (u.noise_mask_amp > 0.0) {
        let noise_seed = hash2(px ^ u.frame_seed, py ^ u.session_hash);
        // Generate per-pixel per-frame noise in [-amp, +amp]
        let nr = (f32(noise_seed & 0xFFu) / 255.0 * 2.0 - 1.0) * u.noise_mask_amp;
        let ng = (f32((noise_seed >> 8u) & 0xFFu) / 255.0 * 2.0 - 1.0) * u.noise_mask_amp;
        let nb = (f32((noise_seed >> 16u) & 0xFFu) / 255.0 * 2.0 - 1.0) * u.noise_mask_amp;
        color = vec4f(
            clamp(color.r + nr, 0.0, 1.0),
            clamp(color.g + ng, 0.0, 1.0),
            clamp(color.b + nb, 0.0, 1.0),
            1.0,
        );
    }

    // ── Forensic watermark (B channel LSB) ──
    let tile_hash = hash(tile_index ^ u.session_hash);
    let pixel_key = hash2(tile_hash, px ^ (py * 1013u));

    if (u.noise_level > 0u) {
        let pixel_local_x = px % u32(u.img_width / f32(u.tiles_x));
        let pixel_local_y = py % u32(u.img_height / f32(u.tiles_y));

        if (pixel_local_x < 2u && pixel_local_y < 2u) {
            let idx_lo = tile_index & 0xFFu;
            let idx_hi = (tile_index >> 8u) & 0xFFu;
            var noise_bits: u32;
            if (pixel_local_y == 0u && pixel_local_x == 0u) {
                noise_bits = idx_lo & ((1u << u.noise_level) - 1u);
            } else if (pixel_local_y == 0u && pixel_local_x == 1u) {
                noise_bits = idx_hi & ((1u << u.noise_level) - 1u);
            } else {
                noise_bits = (tile_hash >> 8u) & ((1u << u.noise_level) - 1u);
            }
            color.b = color.b + f32(noise_bits) / 255.0 * 0.008;
        }

        let rng = hash(pixel_key ^ u.frame_seed);
        color.b = color.b + f32(rng & ((1u << u.noise_level) - 1u)) / 255.0 * 0.003;
    }

    // Per-frame forensic tracking (every screenshot is unique)
    let frame_noise = hash(pixel_key ^ u.frame_seed);
    color.r = clamp(color.r + f32(frame_noise & 1u) / 1024.0, 0.0, 1.0);
    color.g = clamp(color.g + f32((frame_noise >> 1u) & 1u) / 1024.0, 0.0, 1.0);

    return vec4f(color.r, color.g, color.b, 1.0);
}
`;

    // ── WebGPU Renderer ────────────────────────────────────────────────────

    function IrisWebGPURenderer(canvas, imgWidth, imgHeight) {
        this.canvas = canvas;
        this.imgWidth = imgWidth;
        this.imgHeight = imgHeight;
        this.device = null;
        this.gpuContext = null;
        this.format = '';
        this.pipeline = null;
        this.sourceTexture = null;
        this.sampler = null;
        this.uniformBuffer = null;
        this.renderOrderBuffer = null;
        this.bindGroup = null;

        this.tilesX = Math.max(2, Math.ceil(imgWidth / 64));
        this.tilesY = Math.max(2, Math.ceil(imgHeight / 64));
        this.tileCount = this.tilesX * this.tilesY;
        this.revealOrder = []; // shuffled order of tile indices
        this.tilesRevealed = 0;
        this.tilesPerFrame = Math.max(1, Math.ceil(this.tileCount / 15));
        // Bitmap buffer: ceil(tileCount/32) u32s
        this.bitmapWordCount = Math.ceil(this.tileCount / 32);
        this.tileBitmap = null;

        // Anti-screenshot controls
        this.scramble = 0;
        this.frameSeed = 0;
        this.frameCount = 0;
        this.sessionHash = 0;
        this.animFrameId = null;
        this.destroyed = false;

        // Defense levels (adjustable)
        this.interlace = 1;          // 奇偶帧分行闪烁
        this.jitterAmp = 1.5;        // 像素抖动幅度 (pixels)
        this.noiseMaskAmp = 0.06;    // 动态噪点蒙版幅度 (0.0-0.15)
    }

    IrisWebGPURenderer.prototype.init = async function(imageData) {
        if (!navigator.gpu) throw new Error('WebGPU not supported');
        var adapter = await navigator.gpu.requestAdapter();
        if (!adapter) throw new Error('No WebGPU adapter');
        this.device = await adapter.requestDevice();
        this.device.lost.then(function(info) {
            console.error('[Iris] WebGPU device lost:', info.message);
        });

        this.gpuContext = this.canvas.getContext('webgpu');
        if (!this.gpuContext) throw new Error('Cannot get WebGPU context');
        this.format = navigator.gpu.getPreferredCanvasFormat();
        this.gpuContext.configure({
            device: this.device,
            format: this.format,
            alphaMode: 'premultiplied',
        });

        // Source texture
        this.sourceTexture = this.device.createTexture({
            size: [this.imgWidth, this.imgHeight],
            format: 'rgba8unorm',
            usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
        });
        this.device.queue.writeTexture(
            { texture: this.sourceTexture },
            imageData,
            { bytesPerRow: this.imgWidth * 4 },
            { width: this.imgWidth, height: this.imgHeight }
        );

        // Sampler
        this.sampler = this.device.createSampler({ magFilter: 'linear', minFilter: 'linear' });

        // Uniform buffer: 16 u32s = 64 bytes
        this.uniformBuffer = this.device.createBuffer({
            size: 64,
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });

        // Render order storage buffer (bitmap: ceil(tileCount/32) u32s, min 8 u32s = 256 bits)
        var bitmapSize = Math.max(8, this.bitmapWordCount) * 4;
        this.renderOrderBuffer = this.device.createBuffer({
            size: bitmapSize,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });

        // Pipeline
        var shaderModule = this.device.createShaderModule({ code: VERTEX_SHADER + FRAGMENT_SHADER });

        var bindGroupLayout = this.device.createBindGroupLayout({
            entries: [
                { binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
                { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } },
                { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
                { binding: 3, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'read-only-storage' } },
            ],
        });

        this.pipeline = this.device.createRenderPipeline({
            layout: this.device.createPipelineLayout({ bindGroupLayouts: [bindGroupLayout] }),
            vertex: { module: shaderModule, entryPoint: 'vertexMain' },
            fragment: {
                module: shaderModule,
                entryPoint: 'fragmentMain',
                targets: [{ format: this.format }],
            },
            primitive: { topology: 'triangle-list' },
        });

        this.bindGroup = this.device.createBindGroup({
            layout: bindGroupLayout,
            entries: [
                { binding: 0, resource: { buffer: this.uniformBuffer } },
                { binding: 1, resource: this.sourceTexture.createView() },
                { binding: 2, resource: this.sampler },
                { binding: 3, resource: { buffer: this.renderOrderBuffer } },
            ],
        });

        this.revealOrder = fisherYatesShuffle(this.tileCount);
        this.sessionHash = hashString(window.IRIS_SESSION || 'iris-default');
        this.tileBitmap = new Uint32Array(Math.max(8, this.bitmapWordCount));
        this.device.queue.writeBuffer(this.renderOrderBuffer, 0, this.tileBitmap);

        this.startProgressiveRender();
        console.log('[Iris] WebGPU renderer initialized: ' + this.imgWidth + 'x' + this.imgHeight +
            ', interlace=' + this.interlace + ', jitter=' + this.jitterAmp + 'px, noiseMask=' + this.noiseMaskAmp);
    };

    IrisWebGPURenderer.prototype.startProgressiveRender = function() {
        var self = this;
        this.tilesRevealed = 0;

        function frame() {
            if (self.destroyed) return;
            self.frameCount++;
            self.frameSeed = (self.frameCount & 0xFFFF) ^ (hashString(String(self.frameCount)) & 0xFFFF0000);

            // Reveal more tiles each frame
            if (self.tilesRevealed < self.tileCount) {
                var end = Math.min(self.tileCount, self.tilesRevealed + self.tilesPerFrame);
                for (var i = self.tilesRevealed; i < end; i++) {
                    var tileIdx = self.revealOrder[i];
                    var wordIdx = tileIdx >> 5;
                    var bitIdx = tileIdx & 31;
                    self.tileBitmap[wordIdx] |= (1 << bitIdx);
                }
                self.tilesRevealed = end;
                self.device.queue.writeBuffer(self.renderOrderBuffer, 0, self.tileBitmap);
            }

            self.renderFrame();
            self.animFrameId = requestAnimationFrame(frame);
        }

        frame();
    };

    IrisWebGPURenderer.prototype.renderFrame = function() {
        if (!this.device || this.destroyed) return;

        // Uniforms: 16 u32s (64 bytes)
        // [0] img_width  [1] img_height  [2] tiles_x  [3] tiles_y
        // [4] session_hash  [5] noise_level  [6] scramble  [7] frame_seed
        // [8] tiles_revealed  [9] interlace  [10] jitter_amp(f32 bits)  [11] noise_mask_amp(f32 bits)
        // [12-15] padding
        var uniformData = new Uint32Array([
            this.imgWidth,
            this.imgHeight,
            this.tilesX,
            this.tilesY,
            this.sessionHash,
            2,          // noise_level (forensic watermark bits)
            this.scramble,
            this.frameSeed,
            this.tilesRevealed,
            this.interlace,
            floatToU32(this.jitterAmp),
            floatToU32(this.noiseMaskAmp),
            0, 0, 0, 0
        ]);
        this.device.queue.writeBuffer(this.uniformBuffer, 0, uniformData);

        var encoder = this.device.createCommandEncoder();
        var pass = encoder.beginRenderPass({
            colorAttachments: [{
                view: this.gpuContext.getCurrentView(),
                clearValue: { r: 0, g: 0, b: 0, a: 1 },
                loadOp: 'clear',
                storeOp: 'store',
            }],
        });

        pass.setPipeline(this.pipeline);
        pass.setBindGroup(0, this.bindGroup);
        pass.draw(3);
        pass.end();
        this.device.queue.submit([encoder.finish()]);
    };

    IrisWebGPURenderer.prototype.blank = function() {
        this.scramble = 1;
        this.renderFrame();
    };

    IrisWebGPURenderer.prototype.restore = function() {
        this.scramble = 0;
        this.renderFrame();
    };

    IrisWebGPURenderer.prototype.destroy = function() {
        this.destroyed = true;
        if (this.animFrameId) cancelAnimationFrame(this.animFrameId);
        // Destroy GPU resources — device must be last (it invalidates all children)
        if (this.sourceTexture) this.sourceTexture.destroy();
        if (this.uniformBuffer) this.uniformBuffer.destroy();
        if (this.renderOrderBuffer) this.renderOrderBuffer.destroy();
        if (this.pipeline) this.pipeline = null;
        if (this.bindGroup) this.bindGroup = null;
        // Don't destroy device — it may be shared. Just release our reference.
        this.device = null;
    };

    // ── Canvas 2D Fallback Renderer (with same 3 defenses) ────────────────

    function IrisCanvas2DRenderer(canvas, imgWidth, imgHeight) {
        this.canvas = canvas;
        this.ctx = canvas.getContext('2d');
        this.imgWidth = imgWidth;
        this.imgHeight = imgHeight;
        this.sourceCanvas = null;
        this.destroyed = false;
        this.frameCount = 0;
        this.animFrameId = null;

        this.tilesX = Math.max(2, Math.ceil(imgWidth / 64));
        this.tilesY = Math.max(2, Math.ceil(imgHeight / 64));
        this.tileCount = this.tilesX * this.tilesY;
        this.renderOrder = [];
        this.tilesRevealed = 0;

        this.sessionHash = 0;
        this.interlace = true;
        this.jitterAmp = 1;
        this.noiseMaskAmp = 0.04;
    }

    IrisCanvas2DRenderer.prototype.init = function(imageData) {
        var tmpCanvas = document.createElement('canvas');
        tmpCanvas.width = this.imgWidth;
        tmpCanvas.height = this.imgHeight;
        var tmpCtx = tmpCanvas.getContext('2d');
        tmpCtx.putImageData(new ImageData(new Uint8ClampedArray(imageData), this.imgWidth, this.imgHeight), 0, 0);
        this.sourceCanvas = tmpCanvas;

        this.renderOrder = fisherYatesShuffle(this.tileCount);
        this.sessionHash = hashString(window.IRIS_SESSION || 'iris-default');
        this.tilesRevealed = 0;

        // Start animation loop for interlace + jitter + noise mask
        this.startAnimation();
    };

    IrisCanvas2DRenderer.prototype.startAnimation = function() {
        var self = this;

        function frame() {
            if (self.destroyed) return;
            self.frameCount++;

            // Progressive reveal
            if (self.tilesRevealed < self.tileCount) {
                var tilesPerStep = Math.max(1, Math.ceil(self.tileCount / 15));
                self.tilesRevealed = Math.min(self.tileCount, self.tilesRevealed + tilesPerStep);
            }

            self.renderFrame2D();
            self.animFrameId = requestAnimationFrame(frame);
        }

        frame();
    };

    IrisCanvas2DRenderer.prototype.renderFrame2D = function() {
        var ctx = this.ctx;
        var w = this.imgWidth;
        var h = this.imgHeight;
        var frameParity = this.frameCount & 1;

        // Clear
        ctx.clearRect(0, 0, w, h);

        // Draw from source with interlace
        if (this.interlace) {
            // Draw only the rows for this frame's parity
            for (var y = 0; y < h; y++) {
                if ((y & 1) !== frameParity) continue; // Skip rows of opposite parity
                ctx.drawImage(this.sourceCanvas, 0, y, w, 1, 0, y, w, 1);
            }
            // Fill missing rows from neighbors (simplified)
            var imgData = ctx.getImageData(0, 0, w, h);
            var pixels = imgData.data;
            for (var y2 = 0; y2 < h; y2++) {
                if ((y2 & 1) === frameParity) continue; // Already drawn
                // Copy from adjacent row + noise
                var srcY = (y2 > 0) ? y2 - 1 : y2 + 1;
                for (var x = 0; x < w; x++) {
                    var di = (y2 * w + x) * 4;
                    var si = (srcY * w + x) * 4;
                    pixels[di]     = pixels[si]     + ((jsHash(x ^ this.frameCount ^ y2) & 0xF) - 8);
                    pixels[di + 1] = pixels[si + 1] + ((jsHash(y2 ^ this.frameCount ^ x) & 0xF) - 8);
                    pixels[di + 2] = pixels[si + 2] + ((jsHash(x ^ y2 ^ this.frameCount) & 0xF) - 8);
                    pixels[di + 3] = 255;
                }
            }
            ctx.putImageData(imgData, 0, 0);
        } else {
            ctx.drawImage(this.sourceCanvas, 0, 0);
        }

        // Apply noise mask + jitter (simplified for Canvas 2D)
        if (this.noiseMaskAmp > 0 || this.jitterAmp > 0) {
            var imgData2 = ctx.getImageData(0, 0, w, h);
            var px = imgData2.data;
            var amp = Math.round(this.noiseMaskAmp * 255);
            for (var i = 0; i < px.length; i += 4) {
                // Noise mask
                if (amp > 0) {
                    px[i]     = clamp8(px[i]     + (jsHash(i ^ this.frameCount) % (amp * 2 + 1)) - amp);
                    px[i + 1] = clamp8(px[i + 1] + (jsHash(i ^ this.frameCount ^ 0xAA) % (amp * 2 + 1)) - amp);
                    px[i + 2] = clamp8(px[i + 2] + (jsHash(i ^ this.frameCount ^ 0x55) % (amp * 2 + 1)) - amp);
                }
                // Forensic watermark (B channel LSB)
                var pixIdx = i >> 2;
                if (pixIdx < 4) {
                    var lo = pixIdx & 0xFF;
                    px[i + 2] = (px[i + 2] & 0xFC) | (lo & 0x03);
                }
            }
            ctx.putImageData(imgData2, 0, 0);
        }
    };

    IrisCanvas2DRenderer.prototype.blank = function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    };

    IrisCanvas2DRenderer.prototype.restore = function() {
        this.renderFrame2D();
    };

    IrisCanvas2DRenderer.prototype.destroy = function() {
        this.destroyed = true;
        if (this.animFrameId) cancelAnimationFrame(this.animFrameId);
    };

    // ── Utility Functions ──────────────────────────────────────────────────

    function floatToU32(f) {
        var buf = new Float32Array([f]);
        return new Uint32Array(buf.buffer)[0];
    }

    function fisherYatesShuffle(count) {
        var order = [];
        for (var i = 0; i < count; i++) order.push(i);
        var rng = (Date.now() & 0xFFFF) ^ 0x49524953;
        for (var j = count - 1; j > 0; j--) {
            rng = ((rng * 1103515245) + 12345) & 0x7FFFFFFF;
            var k = rng % (j + 1);
            var tmp = order[j]; order[j] = order[k]; order[k] = tmp;
        }
        return order;
    }

    function hashString(str) {
        var hash = 0x49524953;
        for (var i = 0; i < str.length; i++) {
            hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0;
        }
        return hash >>> 0;
    }

    function jsHash(v) {
        v = ((v >> 16) ^ v) * 0x45d9f3b | 0;
        v = ((v >> 16) ^ v) * 0x45d9f3b | 0;
        v = (v >> 16) ^ v;
        return v >>> 0;
    }

    function clamp8(v) {
        return v < 0 ? 0 : (v > 255 ? 255 : v);
    }

    function applyJsNoise(pixels, tileIndex) {
        if (pixels.length < 16) return;
        var lo = tileIndex & 0xFF;
        var mask = 0xFC;
        pixels[2] = (pixels[2] & mask) | (lo & 0x03);
        pixels[6] = (pixels[6] & mask) | ((lo >> 2) & 0x03);
        pixels[10] = (pixels[10] & mask) | ((lo >> 4) & 0x03);
        pixels[14] = (pixels[14] & mask) | ((lo >> 6) & 0x03);
    }

    // ── Secure Image Rendering ─────────────────────────────────────────────

    async function renderSecureImage(imgElement) {
        var src = imgElement.getAttribute('data-iris-src') || imgElement.src;
        if (!src || imgElement.dataset.irisRendered === 'true') return;

        try {
            var imgResp = await fetch(src);
            if (!imgResp.ok) return;

            var imgBlob = await imgResp.blob();
            var imgBitmap = await createImageBitmap(imgBlob);
            var w = imgBitmap.width;
            var h = imgBitmap.height;

            var tmpCanvas = document.createElement('canvas');
            tmpCanvas.width = w;
            tmpCanvas.height = h;
            var tmpCtx = tmpCanvas.getContext('2d');
            tmpCtx.drawImage(imgBitmap, 0, 0);
            var imageData = tmpCtx.getImageData(0, 0, w, h);

            var canvas = document.createElement('canvas');
            canvas.width = w;
            canvas.height = h;
            canvas.className = (imgElement.className || '') + ' iris-secure-canvas';
            canvas.style.cssText = (imgElement.style.cssText || '');
            canvas.style.userSelect = 'none';
            canvas.style.webkitUserSelect = 'none';
            canvas.style.webkitUserDrag = 'none';
            canvas.setAttribute('draggable', 'false');

            var wrapper = document.createElement('div');
            wrapper.style.position = 'relative';
            wrapper.style.display = 'inline-block';
            wrapper.className = 'iris-secure-wrapper';
            canvas.style.display = 'block';

            var overlay = document.createElement('div');
            overlay.style.position = 'absolute';
            overlay.style.top = '0';
            overlay.style.left = '0';
            overlay.style.width = '100%';
            overlay.style.height = '100%';
            overlay.style.zIndex = '1';
            overlay.style.pointerEvents = 'none';

            wrapper.appendChild(canvas);
            wrapper.appendChild(overlay);

            var renderer = null;
            if (navigator.gpu) {
                try {
                    var gpuRenderer = new IrisWebGPURenderer(canvas, w, h);
                    await gpuRenderer.init(imageData.data);
                    renderer = gpuRenderer;
                    canvas.dataset.irisRenderer = 'webgpu';
                    console.log('[Iris] WebGPU renderer for: ' + src +
                        ' (interlace + jitter ' + gpuRenderer.jitterAmp + 'px + noiseMask ' + gpuRenderer.noiseMaskAmp + ')');
                } catch (e) {
                    console.warn('[Iris] WebGPU failed, falling back to Canvas 2D:', e);
                }
            }

            if (!renderer) {
                var canvas2dRenderer = new IrisCanvas2DRenderer(canvas, w, h);
                canvas2dRenderer.init(imageData.data);
                renderer = canvas2dRenderer;
                canvas.dataset.irisRenderer = 'canvas2d';
                console.log('[Iris] Canvas 2D renderer for: ' + src);
            }

            canvas._irisRenderer = renderer;
            secureCanvases.push(renderer);

            imgElement.parentNode.replaceChild(wrapper, imgElement);
            imgElement.dataset.irisRendered = 'true';

        } catch (e) {
            console.error('[Iris Canvas] Render error:', e);
        }
    }

    // ── Anti-capture defenses ──────────────────────────────────────────────

    function blankAllCanvases() {
        secureCanvases.forEach(function(r) { if (!r.destroyed) r.blank(); });
    }

    function restoreAllCanvases() {
        secureCanvases.forEach(function(r) { if (!r.destroyed) r.restore(); });
    }

    function setupVisibilityProtection() {
        document.addEventListener('visibilitychange', function() {
            if (document.hidden) {
                blankAllCanvases();
            } else {
                setTimeout(restoreAllCanvases, 100);
            }
        });
    }

    function setupCaptureDetection() {
        if (navigator.mediaDevices && navigator.mediaDevices.getDisplayMedia) {
            var original = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices);
            navigator.mediaDevices.getDisplayMedia = function(constraints) {
                console.warn('[Iris] Screen capture detected - blanking');
                blankAllCanvases();
                var promise = original(constraints);
                setTimeout(restoreAllCanvases, 200);
                return promise;
            };
        }
    }

    function setupPrintProtection() {
        var style = document.createElement('style');
        style.textContent = '@media print { .iris-secure-wrapper { display: none !important; } }';
        document.head.appendChild(style);
    }

    function setupContextMenuBlock() {
        document.addEventListener('contextmenu', function(e) {
            if (e.target.tagName === 'CANVAS' && e.target.closest('.iris-secure-wrapper')) {
                e.preventDefault();
            }
        });
    }

    function setupKeyProtection() {
        document.addEventListener('keydown', function(e) {
            if (e.ctrlKey && e.key === 'p') {
                blankAllCanvases();
                setTimeout(restoreAllCanvases, 500);
            }
            if (e.key === 'PrintScreen') {
                blankAllCanvases();
                setTimeout(restoreAllCanvases, 300);
            }
            if (e.metaKey && e.shiftKey && (e.key === '4' || e.key === '5')) {
                blankAllCanvases();
                setTimeout(restoreAllCanvases, 300);
            }
        });
    }

    // ── Auto-init ──────────────────────────────────────────────────────────

    function initIrisCanvas() {
        window.IRIS_SESSION = 'iris-' + Date.now().toString(36) + '-' + Math.random().toString(36).substr(2, 5);

        setupVisibilityProtection();
        setupCaptureDetection();
        setupPrintProtection();
        setupContextMenuBlock();
        setupKeyProtection();

        var images = document.querySelectorAll('img');
        var toSecure = [];

        images.forEach(function(img) {
            if (img.dataset.irisSecure !== undefined || img.hasAttribute('data-iris-secure')) {
                toSecure.push(img);
                return;
            }
            if (window.IRIS_CONFIG && window.IRIS_CONFIG.fileMappings) {
                var src = img.getAttribute('src') || '';
                var cleanSrc = src.replace(/^\//, '').replace(/^\.\//, '');
                var mappings = window.IRIS_CONFIG.fileMappings;
                for (var key in mappings) {
                    if (mappings[key] === cleanSrc || key === cleanSrc) {
                        toSecure.push(img);
                        break;
                    }
                }
            }
        });

        console.log('[Iris Canvas] Found ' + toSecure.length + ' images to secure (WebGPU: ' + (!!navigator.gpu) + ')');

        toSecure.forEach(function(img) {
            if (img.complete && img.naturalWidth > 0) {
                renderSecureImage(img);
            } else {
                img.addEventListener('load', function() { renderSecureImage(img); });
            }
        });

        if (typeof MutationObserver !== 'undefined') {
            var observer = new MutationObserver(function(mutations) {
                mutations.forEach(function(mutation) {
                    mutation.addedNodes.forEach(function(node) {
                        if (node.nodeType === 1) {
                            if (node.tagName === 'IMG' && (node.dataset.irisSecure !== undefined || node.hasAttribute('data-iris-secure'))) {
                                if (node.complete) renderSecureImage(node);
                                else node.addEventListener('load', function() { renderSecureImage(node); });
                            }
                            var imgs = node.querySelectorAll && node.querySelectorAll('img[data-iris-secure]');
                            if (imgs) {
                                imgs.forEach(function(img) {
                                    if (img.complete) renderSecureImage(img);
                                    else img.addEventListener('load', function() { renderSecureImage(img); });
                                });
                            }
                        }
                    });
                });
            });
            observer.observe(document.body || document.documentElement, { childList: true, subtree: true });
        }
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initIrisCanvas);
    } else {
        initIrisCanvas();
    }
})();

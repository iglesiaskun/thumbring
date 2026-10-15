//! Draws a ring [`Scene`] on a transparent window.
//!
//! - Direct3D 11 draws the shapes with the SDF shader (`ring.hlsl`): the
//!   items, the highlights, the blob and the center button, in the colors of
//!   the appearance.
//! - Direct2D draws the icons (Lucide SVGs, or SVG and PNG files of the
//!   user's own) and the label on top. PNG files are decoded with WIC.
//! - DirectComposition puts the result on the screen with per-pixel
//!   transparency, without a redirection bitmap.
//!
//! The swap chain has a frame latency object: the caller waits on it (with its
//! messages and timers) before drawing each frame, so presenting never blocks
//! the thread and one frame at most is in flight.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::mem::ManuallyDrop;
use std::path::Path;

use thumbring_core::anim::{Blob, Circle};
use thumbring_core::color::Color;
use thumbring_core::config::{Appearance, LabelStyle};
use thumbring_core::layout::{ITEM_DIAMETER_PX, Vec2};
use thumbring_core::ring::{IconFormat, IconSource, Ring, Slot};
use thumbring_core::ring_machine::Target;
use thumbring_core::screen::Rect;
use thumbring_core::view::{
    IconDraw, IconKey, ItemKey, LabelDraw, MAX_DARK, MAX_WHITE, Scene, label_position,
};
use windows::Win32::Foundation::{CloseHandle, E_INVALIDARG, GENERIC_READ, HANDLE, HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_LAYER_OPTIONS1_NONE,
    D2D1_LAYER_PARAMETERS1, D2D1_ROUNDED_RECT, D2D1CreateFactory, ID2D1Bitmap, ID2D1DeviceContext5,
    ID2D1Factory1, ID2D1SolidColorBrush, ID2D1SvgDocument, ID2D1SvgGlyphStyle,
};
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
    D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAP_WRITE_DISCARD,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DYNAMIC,
    D3D11_USAGE_STAGING, D3D11_VIEWPORT, D3D11CreateDevice, ID3D11Buffer, ID3D11Device,
    ID3D11DeviceContext, ID3D11PixelShader, ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
    IDXGISwapChain2,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::WaitForSingleObjectEx;
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{HSTRING, Interface, PCSTR, s};
use windows_numerics::{Matrix3x2, Vector2};

use super::icons;
use crate::log::warning;

const SHADER: &str = include_str!("ring.hlsl");

/// The swap chain hands out a frame latency object (see the module docs).
const SWAP_CHAIN_FLAGS: DXGI_SWAP_CHAIN_FLAG = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;

/// Icon size as a fraction of the item diameter, and stroke width.
const ITEM_ICON: (f64, f32) = (0.40, 1.9);
/// The × in the center is smaller and bolder.
const CENTER_ICON: (f64, f32) = (0.26, 2.4);
/// Text size of the first letter that stands in for a missing icon: 15 px
/// on a 52 px item.
const LETTER: f64 = 15.0 / ITEM_DIAMETER_PX;
/// Strength of the soft shadow around the items, when it is on.
const SHADOW: f32 = 0.10;
/// PNG files wider or taller than this are refused: decoding one takes
/// memory and time, and an icon is drawn at a few dozen pixels.
const MAX_PNG_SIDE: u32 = 2048;
/// Plain text: no SVG glyph style for color fonts.
const NO_GLYPH_STYLE: Option<&ID2D1SvgGlyphStyle> = None;

/// Mirrors the `cbuffer Scene` of the shader, field by field.
#[repr(C)]
#[derive(Clone, Copy)]
struct Constants {
    white: [[f32; 4]; MAX_WHITE],
    dark: [[f32; 4]; MAX_DARK],
    center: [f32; 4],
    blob: [f32; 4],
    blob_shape: [f32; 4],
    hero: [f32; 4],
    hero_shape: [f32; 4],
    dark_color: [f32; 4],
    item_color: [f32; 4],
    cancel_color: [f32; 4],
    params: [f32; 4],
    params2: [f32; 4],
    bounds: [f32; 4],
    viewport: [f32; 4],
}

/// Text of the label: the item name and, smaller, what it does.
pub struct LabelText {
    pub name: String,
    pub detail: Option<String>,
}

/// Text formats of the label, in pixels for the monitor scale.
struct Fonts {
    name: IDWriteTextFormat,
    detail: IDWriteTextFormat,
}

impl Fonts {
    fn new(dwrite: &IDWriteFactory, scale: f64) -> windows::core::Result<Self> {
        let px = |at_100: f64| (at_100 * scale) as f32;
        Ok(Self {
            name: text_format(dwrite, "Segoe UI", DWRITE_FONT_WEIGHT_MEDIUM, px(13.0))?,
            detail: text_format(dwrite, "Consolas", DWRITE_FONT_WEIGHT_NORMAL, px(11.0))?,
        })
    }
}

struct Brushes {
    pill: ID2D1SolidColorBrush,
    shadow: ID2D1SolidColorBrush,
    name: ID2D1SolidColorBrush,
    detail: ID2D1SolidColorBrush,
    ink: ID2D1SolidColorBrush,
    ink_on: ID2D1SolidColorBrush,
}

impl Brushes {
    fn new(d2d: &ID2D1DeviceContext5, look: &Appearance) -> windows::core::Result<Self> {
        let brush = |[r, g, b]: [f32; 3], a: f32| {
            // SAFETY: a plain factory call on the device context.
            unsafe { d2d.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a }, None) }
        };
        let hex = |r, g, b| Color::rgb(r, g, b).to_f32();
        let (pill, name, detail) = match look.label {
            LabelStyle::Light => (
                Color::WHITE.to_f32(),
                hex(0x24, 0x25, 0x2B),
                hex(0x6A, 0x6E, 0x79),
            ),
            LabelStyle::Dark => (
                hex(0x18, 0x19, 0x1E),
                hex(0xF1, 0xF2, 0xF5),
                hex(0xA6, 0xAA, 0xB6),
            ),
        };
        Ok(Self {
            pill: brush(pill, 0.92)?,
            shadow: brush([0.0; 3], 0.035)?,
            name: brush(name, 1.0)?,
            detail: brush(detail, 1.0)?,
            ink: brush(look.icon_color.to_f32(), 1.0)?,
            ink_on: brush(look.highlight_icon_color.to_f32(), 1.0)?,
        })
    }
}

/// What an icon is drawn with.
#[derive(Clone)]
enum Picture {
    Svg(ID2D1SvgDocument),
    /// A PNG, already scaled, and where it goes in the icon square.
    Bitmap(ID2D1Bitmap, D2D_RECT_F),
    /// No picture: the first letter of the item name.
    Letter,
}

pub struct Renderer {
    width: u32,
    height: u32,
    /// Monitor scale (1.0 = 100%), for text and shadow sizes.
    scale: f64,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    /// Signaled when the swap chain can take a new frame.
    frame_waiter: HANDLE,
    composition: IDCompositionDevice,
    // Kept alive: dropping them would take the content off the screen.
    _target: (IDCompositionTarget, IDCompositionVisual),
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    constants: ID3D11Buffer,
    d2d: ID2D1DeviceContext5,
    dwrite: IDWriteFactory,
    fonts: Fonts,
    /// The first letter of an item without an icon, for an item diameter
    /// (in its bits): it grows with the item.
    letter: Option<(u64, IDWriteTextFormat)>,
    appearance: Appearance,
    brushes: Brushes,
    /// Icons by (icon, drawn on the highlight, is the center, size bits).
    pictures: HashMap<(String, bool, bool, u32), Picture>,
    /// Icon files that could not be drawn, already told in the log.
    broken: HashSet<String>,
}

impl Renderer {
    /// Creates everything to draw on `window`, which is `width` by `height`
    /// physical pixels on a monitor with this scale.
    pub fn new(window: HWND, width: u32, height: u32, scale: f64) -> windows::core::Result<Self> {
        let (device, context) = create_device()?;
        let dxgi_device: IDXGIDevice = device.cast()?;
        // SAFETY: COM calls on objects created above; every out pointer is a
        // local or a field that lives as long as the renderer.
        unsafe {
            let factory: IDXGIFactory2 = dxgi_device.GetAdapter()?.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: SWAP_CHAIN_FLAGS.0 as u32,
                ..Default::default()
            };
            let swap_chain = factory.CreateSwapChainForComposition(&device, &desc, None)?;
            // One frame in flight at most: each frame shows the ring as it is
            // when drawn, not a few refreshes later.
            let latency: IDXGISwapChain2 = swap_chain.cast()?;
            latency.SetMaximumFrameLatency(1)?;
            let frame_waiter = latency.GetFrameLatencyWaitableObject();

            let composition: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let target = composition.CreateTargetForHwnd(window, true)?;
            let visual = composition.CreateVisual()?;
            visual.SetContent(&swap_chain)?;
            target.SetRoot(&visual)?;
            composition.Commit()?;

            let (vertex_shader, pixel_shader) = compile_shaders(&device)?;
            let constants = create_constant_buffer(&device)?;

            let factory2d: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d: ID2D1DeviceContext5 = factory2d
                .CreateDevice(&dxgi_device)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
                .cast()?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let fonts = Fonts::new(&dwrite, scale)?;
            let appearance = Appearance::default();
            let brushes = Brushes::new(&d2d, &appearance)?;

            Ok(Self {
                width,
                height,
                scale,
                device,
                context,
                swap_chain,
                frame_waiter,
                composition,
                _target: (target, visual),
                vertex_shader,
                pixel_shader,
                constants,
                d2d,
                dwrite,
                fonts,
                letter: None,
                appearance,
                brushes,
                pictures: HashMap::new(),
                broken: HashSet::new(),
            })
        }
    }

    /// Takes the appearance of new settings. Every icon is made again, so
    /// the icon files are read again too.
    pub fn set_appearance(&mut self, appearance: &Appearance) -> windows::core::Result<()> {
        self.brushes = Brushes::new(&self.d2d, appearance)?;
        self.appearance = appearance.clone();
        self.pictures.clear();
        self.broken.clear();
        Ok(())
    }

    /// Changes the window size and the monitor scale, when the window moves
    /// to another monitor.
    pub fn resize(&mut self, width: u32, height: u32, scale: f64) -> windows::core::Result<()> {
        if (width, height) != (self.width, self.height) {
            // SAFETY: every reference to the back buffers is released at the
            // end of each frame; ClearState drops the ones the context keeps.
            unsafe {
                self.context.ClearState();
                self.context.Flush();
                self.swap_chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_UNKNOWN,
                    SWAP_CHAIN_FLAGS,
                )?;
            }
            self.width = width;
            self.height = height;
        }
        if scale != self.scale {
            self.fonts = Fonts::new(&self.dwrite, scale)?;
            // Icon sizes follow the item size, which follows the scale.
            self.pictures.clear();
            self.scale = scale;
        }
        Ok(())
    }

    /// Signaled when the swap chain can take a new frame. Each wait that
    /// returns it must be followed by one [`Self::draw`].
    pub fn frame_waiter(&self) -> HANDLE {
        self.frame_waiter
    }

    /// Waits until the swap chain can take a new frame (at most one second).
    pub fn wait_for_frame(&self) {
        // SAFETY: waits on a handle owned by the renderer.
        unsafe {
            WaitForSingleObjectEx(self.frame_waiter, 1000, true);
        }
    }

    /// Time between two screen refreshes, as DirectComposition sees it.
    pub fn refresh_period_ms(&self) -> f64 {
        // SAFETY: a query on the device owned by the renderer.
        let stats = unsafe { self.composition.GetFrameStatistics() };
        match stats.map(|s| s.currentCompositionRate) {
            Ok(rate) if rate.Numerator > 0 => {
                1000.0 * f64::from(rate.Denominator) / f64::from(rate.Numerator)
            }
            _ => 1000.0 / 60.0,
        }
    }

    /// Draws, without presenting it, a frame with every icon of the ring in
    /// both colors and a label. The first real frame then finds the shader,
    /// the icons and the fonts ready, instead of stuttering on them.
    pub fn warm_up(&mut self, ring: &Ring, item_d: f64) -> windows::core::Result<()> {
        let at = Vec2::new(item_d, item_d);
        let r = item_d / 2.0;
        let mut keys = vec![IconKey::Center];
        let mut runnable = None;
        for (slot, content) in (0u8..).zip(&ring.slots) {
            match content {
                Slot::Empty => continue,
                Slot::Action(_) => {
                    runnable.get_or_insert(Target::Action { slot });
                }
                Slot::Folder { items, .. } => {
                    for index in (0u8..).take(items.len()) {
                        keys.push(IconKey::Item(ItemKey::Child { slot, index }));
                        runnable.get_or_insert(Target::Child { slot, index });
                    }
                }
            }
            keys.push(IconKey::Item(ItemKey::Slot(slot)));
        }
        let icons = keys
            .into_iter()
            .flat_map(|key| {
                // Half opacity on the white ones also prepares the fade layers.
                [(false, 1.0), (true, 0.5)].map(|(on, opacity)| IconDraw {
                    key,
                    center: at,
                    scale: 1.0,
                    opacity,
                    on,
                })
            })
            .collect();
        let circle = Circle { center: at, r };
        let scene = Scene {
            ring_center: at,
            alpha: 1.0,
            white: vec![circle],
            dark: vec![circle],
            blob: None,
            hero: None,
            center: circle,
            center_red: 0.0,
            k_white: 0.22 * r,
            k_dark: 0.22 * r,
            highlight_fade: 0.0,
            icons,
            label: runnable.map(|target| LabelDraw {
                target,
                item: at,
                item_r: r,
                opacity: 1.0,
            }),
            active: false,
        };
        let text = runnable.and_then(|target| label_text(ring, target));
        self.render(Some(&scene), ring, item_d, text.as_ref())
    }

    /// Draws one frame and presents it at the next screen refresh. `None`
    /// clears the window. `item_d` is the item diameter in pixels.
    pub fn draw(
        &mut self,
        scene: Option<&Scene>,
        ring: &Ring,
        item_d: f64,
        label: Option<&LabelText>,
    ) -> windows::core::Result<()> {
        self.render(scene, ring, item_d, label)?;
        self.present()
    }

    /// Shows the frame drawn by [`Self::render`] at the next screen refresh.
    pub fn present(&self) -> windows::core::Result<()> {
        // SAFETY: presents the swap chain owned by the renderer.
        unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)).ok() }
    }

    /// Draws one frame into the back buffer, without showing it yet.
    pub fn render(
        &mut self,
        scene: Option<&Scene>,
        ring: &Ring,
        item_d: f64,
        label: Option<&LabelText>,
    ) -> windows::core::Result<()> {
        // SAFETY: COM calls on objects owned by the renderer; the back buffer
        // and its views are released before the next frame.
        unsafe {
            let back: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
            let mut target = None;
            self.device
                .CreateRenderTargetView(&back, None, Some(&mut target))?;
            let target = target.expect("CreateRenderTargetView returned a view");
            self.context.ClearRenderTargetView(&target, &[0.0; 4]);

            if let Some(scene) = scene {
                self.upload(scene)?;
                self.context
                    .OMSetRenderTargets(Some(&[Some(target.clone())]), None);
                self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: self.width as f32,
                    Height: self.height as f32,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]));
                self.context
                    .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
                self.context.IASetInputLayout(None);
                self.context.VSSetShader(&self.vertex_shader, None);
                self.context.PSSetShader(&self.pixel_shader, None);
                let buffers = [Some(self.constants.clone())];
                self.context.VSSetConstantBuffers(0, Some(&buffers));
                self.context.PSSetConstantBuffers(0, Some(&buffers));
                self.context.Draw(4, 0);
                self.context.OMSetRenderTargets(None, None);

                let surface: IDXGISurface = back.cast()?;
                let props = D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                    colorContext: ManuallyDrop::new(None),
                };
                let bitmap = self
                    .d2d
                    .CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
                self.d2d.SetTarget(&bitmap);
                self.d2d.BeginDraw();
                for icon in &scene.icons {
                    self.draw_icon(icon, ring, item_d)?;
                }
                if let (Some(draw), Some(text)) = (&scene.label, label) {
                    self.draw_label(draw, scene.ring_center, text)?;
                }
                self.d2d.SetTransform(&Matrix3x2::identity());
                let ended = self.d2d.EndDraw(None, None);
                self.d2d.SetTarget(None);
                ended?;
            }
            drop(target);
            drop(back);
            Ok(())
        }
    }

    /// Pixels of the frame drawn by [`Self::render`], before it is presented:
    /// rows of BGRA, top to bottom, with premultiplied alpha.
    pub fn read_pixels(&self) -> windows::core::Result<(u32, u32, Vec<u8>)> {
        // SAFETY: copies the back buffer into a CPU-readable texture of the
        // same size and format, then reads it row by row while mapped.
        unsafe {
            let back: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            back.GetDesc(&mut desc);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.expect("staging texture");
            self.context.CopyResource(&staging, &back);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let row = self.width as usize * 4;
            let mut pixels = Vec::with_capacity(row * self.height as usize);
            for y in 0..self.height as usize {
                let start = (mapped.pData as *const u8).add(y * mapped.RowPitch as usize);
                pixels.extend_from_slice(std::slice::from_raw_parts(start, row));
            }
            self.context.Unmap(&staging, 0);
            Ok((self.width, self.height, pixels))
        }
    }

    /// Fills the shader constants from the scene and the appearance.
    unsafe fn upload(&self, scene: &Scene) -> windows::core::Result<()> {
        let look = &self.appearance;
        let item = look.item_color.to_f32();
        let fade = scene.highlight_fade as f32;
        let dark = look.highlight_color.to_f32();
        let dark = [0, 1, 2].map(|i| dark[i] + (item[i] - dark[i]) * fade);
        let cancel = look.cancel_color.to_f32();
        let mut c = Constants {
            white: [[0.0; 4]; MAX_WHITE],
            dark: [[0.0; 4]; MAX_DARK],
            center: [
                scene.center.center.x as f32,
                scene.center.center.y as f32,
                scene.center.r as f32,
                scene.center_red as f32,
            ],
            blob: [0.0; 4],
            blob_shape: [1.0, 0.0, 1.0, 1.0],
            hero: [0.0; 4],
            hero_shape: [1.0, 0.0, 1.0, 1.0],
            dark_color: [dark[0], dark[1], dark[2], scene.alpha as f32],
            item_color: [item[0], item[1], item[2], look.item_opacity as f32],
            cancel_color: [cancel[0], cancel[1], cancel[2], 0.0],
            params: [
                scene.k_white as f32,
                scene.k_dark as f32,
                scene.white.len().min(MAX_WHITE) as f32,
                scene.dark.len().min(MAX_DARK) as f32,
            ],
            params2: [
                (14.0 * self.scale) as f32,
                if look.shadow { SHADOW } else { 0.0 },
                0.0,
                0.0,
            ],
            bounds: [0.0; 4],
            viewport: [self.width as f32, self.height as f32, 0.0, 0.0],
        };
        for (slot, circle) in c.white.iter_mut().zip(&scene.white) {
            *slot = [
                circle.center.x as f32,
                circle.center.y as f32,
                circle.r as f32,
                0.0,
            ];
        }
        for (slot, circle) in c.dark.iter_mut().zip(&scene.dark) {
            *slot = [
                circle.center.x as f32,
                circle.center.y as f32,
                circle.r as f32,
                0.0,
            ];
        }
        if let Some(b) = scene.blob {
            (c.blob, c.blob_shape) = blob_constants(&b);
        }
        if let Some(h) = scene.hero {
            (c.hero, c.hero_shape) = blob_constants(&h);
        }
        c.bounds = self.bounds(scene);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the mapped buffer is exactly `size_of::<Constants>()` bytes.
        unsafe {
            self.context.Map(
                &self.constants,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            )?;
            std::ptr::copy_nonoverlapping(&c, mapped.pData as *mut Constants, 1);
            self.context.Unmap(&self.constants, 0);
        }
        Ok(())
    }

    /// The part of the window the shader needs to touch: every shape, its
    /// shadow and the smooth union margin.
    fn bounds(&self, scene: &Scene) -> [f32; 4] {
        let center = scene.ring_center;
        let mut extent = (scene.center.center - center).length() + scene.center.r;
        for circle in scene.white.iter().chain(&scene.dark) {
            extent = extent.max((circle.center - center).length() + circle.r);
        }
        for blob in scene.blob.iter().chain(&scene.hero) {
            let reach = blob.r * blob.stretch.max(blob.squash);
            extent = extent.max((blob.center - center).length() + reach);
        }
        extent += 14.0 * self.scale + scene.k_white.max(scene.k_dark) + 2.0;
        let (w, h) = (f64::from(self.width), f64::from(self.height));
        [
            (center.x - extent).clamp(0.0, w) as f32,
            (center.y - extent).clamp(0.0, h) as f32,
            (center.x + extent).clamp(0.0, w) as f32,
            (center.y + extent).clamp(0.0, h) as f32,
        ]
    }

    unsafe fn draw_icon(
        &mut self,
        icon: &IconDraw,
        ring: &Ring,
        item_d: f64,
    ) -> windows::core::Result<()> {
        if icon.opacity <= 0.01 || icon.scale <= 0.01 {
            return Ok(());
        }
        let is_center = icon.key == IconKey::Center;
        let (fraction, stroke) = if is_center { CENTER_ICON } else { ITEM_ICON };
        let size = item_d * fraction;
        let (name, glyph) = match icon.key {
            IconKey::Center => ("x".to_owned(), "×".to_owned()),
            IconKey::Item(key) => match item_of(ring, key) {
                Some((name, icon)) => (
                    icon.unwrap_or_default().to_owned(),
                    name.chars().next().map(String::from).unwrap_or_default(),
                ),
                None => return Ok(()),
            },
        };
        let s = icon.scale as f32;
        let half = (size / 2.0) as f32 * s;
        // SAFETY: Direct2D calls between BeginDraw and EndDraw.
        unsafe {
            let picture = self.picture(&name, icon.on, is_center, size, stroke)?;
            let layer = icon.opacity < 0.999;
            if layer {
                self.push_opacity(icon.opacity as f32)?;
            }
            self.d2d.SetTransform(&Matrix3x2 {
                M11: s,
                M12: 0.0,
                M21: 0.0,
                M22: s,
                M31: icon.center.x as f32 - half,
                M32: icon.center.y as f32 - half,
            });
            match picture {
                Picture::Svg(document) => self.d2d.DrawSvgDocument(&document),
                Picture::Bitmap(bitmap, place) => self.d2d.DrawBitmap(
                    &bitmap,
                    Some(&place),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                ),
                Picture::Letter => self.draw_letter(&glyph, icon.on, size as f32, item_d)?,
            }
            self.d2d.SetTransform(&Matrix3x2::identity());
            if layer {
                self.d2d.PopLayer();
            }
        }
        Ok(())
    }

    /// What an icon is drawn with, made once per color and size. A file
    /// that cannot be drawn gives the first letter, and a line in the log.
    unsafe fn picture(
        &mut self,
        name: &str,
        on: bool,
        is_center: bool,
        size: f64,
        stroke: f32,
    ) -> windows::core::Result<Picture> {
        let key = (name.to_owned(), on, is_center, (size as f32).to_bits());
        if let Some(cached) = self.pictures.get(&key) {
            return Ok(cached.clone());
        }
        let look = &self.appearance;
        let color = if on {
            look.highlight_icon_color
        } else {
            look.icon_color
        }
        .to_string();
        let picture = match IconSource::of(name) {
            // SAFETY: Direct2D calls on the device context of the renderer.
            Some(IconSource::Builtin(name)) => {
                match icons::svg(name, &color, stroke, size as f32) {
                    Some(text) => Picture::Svg(unsafe { self.svg_document(&text, size)? }),
                    None => Picture::Letter,
                }
            }
            // SAFETY: as above, plus WIC for PNG files.
            Some(IconSource::File(path, format)) => {
                match unsafe { self.icon_file(path, format, &color, size) } {
                    Ok(picture) => picture,
                    Err(reason) => {
                        if self.broken.insert(path.to_owned()) {
                            warning!(
                                "ring: the icon {path} cannot be drawn ({reason}); the item shows \
                                 the first letter of its name"
                            );
                        }
                        Picture::Letter
                    }
                }
            }
            None => Picture::Letter,
        };
        self.pictures.insert(key, picture.clone());
        Ok(picture)
    }

    unsafe fn svg_document(
        &self,
        text: &str,
        size: f64,
    ) -> windows::core::Result<ID2D1SvgDocument> {
        // SAFETY: the stream copies the bytes; the viewport is the icon size.
        unsafe {
            let stream = SHCreateMemStream(Some(text.as_bytes()));
            let viewport = D2D_SIZE_F {
                width: size as f32,
                height: size as f32,
            };
            self.d2d.CreateSvgDocument(stream.as_ref(), viewport)
        }
    }

    /// The picture of an icon file of the user's own, `size` pixels wide.
    unsafe fn icon_file(
        &self,
        path: &str,
        format: IconFormat,
        color: &str,
        size: f64,
    ) -> Result<Picture, String> {
        // Checked again: the file may have changed since the settings loaded.
        let file = icons::file(path)?;
        let shown = file.display();
        match format {
            IconFormat::Svg => {
                let source = fs::read_to_string(&file)
                    .map_err(|e| format!("could not read {shown}: {e}"))?;
                let text = icons::user_svg(&source, color, size as f32)
                    .ok_or_else(|| format!("{shown} has no <svg> element"))?;
                // SAFETY: a Direct2D call on the device context.
                unsafe { self.svg_document(&text, size) }
                    .map(Picture::Svg)
                    .map_err(|e| format!("Direct2D could not read {shown} as SVG: {}", e.message()))
            }
            // SAFETY: WIC and Direct2D calls, with COM initialized around them.
            IconFormat::Png => unsafe { self.png(&file, size) }
                .map_err(|e| format!("could not read {shown} as PNG: {}", e.message())),
        }
    }

    /// A PNG file scaled to fit a `size` square, keeping its proportions.
    unsafe fn png(&self, file: &Path, size: f64) -> windows::core::Result<Picture> {
        // WIC needs COM on this thread.
        // SAFETY: initialized and uninitialized on this same thread; every
        // WIC object is released before the end of `decode`.
        unsafe {
            let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            let picture = self.decode_png(file, size);
            if com.is_ok() {
                CoUninitialize();
            }
            picture
        }
    }

    unsafe fn decode_png(&self, file: &Path, size: f64) -> windows::core::Result<Picture> {
        // SAFETY: WIC and Direct2D calls on objects created here; the out
        // pointers are locals.
        unsafe {
            let wic: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let decoder = wic.CreateDecoderFromFilename(
                &HSTRING::from(file.as_os_str()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let (mut width, mut height) = (0, 0);
            frame.GetSize(&mut width, &mut height)?;
            if width == 0 || height == 0 || width.max(height) > MAX_PNG_SIDE {
                return Err(windows::core::Error::new(
                    E_INVALIDARG,
                    format!(
                        "it is {width} × {height} pixels; icons go up to {MAX_PNG_SIDE} × {MAX_PNG_SIDE}"
                    ),
                ));
            }
            // Premultiplied first, so scaling does not bleed the color of
            // transparent pixels into the edges.
            let premultiplied = wic.CreateFormatConverter()?;
            premultiplied.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let fit = size / f64::from(width.max(height));
            let scaled_size = |v: u32| ((f64::from(v) * fit).round() as u32).max(1);
            let (w, h) = (scaled_size(width), scaled_size(height));
            let scaler = wic.CreateBitmapScaler()?;
            scaler.Initialize(&premultiplied, w, h, WICBitmapInterpolationModeFant)?;
            let bitmap = self.d2d.CreateBitmapFromWicBitmap(&scaler, None)?;
            let left = ((size - f64::from(w)) / 2.0) as f32;
            let top = ((size - f64::from(h)) / 2.0) as f32;
            let place = D2D_RECT_F {
                left,
                top,
                right: left + w as f32,
                bottom: top + h as f32,
            };
            Ok(Picture::Bitmap(bitmap.cast()?, place))
        }
    }

    /// First letter of the item name, when it has no icon.
    unsafe fn draw_letter(
        &mut self,
        letter: &str,
        on: bool,
        size: f32,
        item_d: f64,
    ) -> windows::core::Result<()> {
        let format = match &self.letter {
            Some((bits, format)) if *bits == item_d.to_bits() => format.clone(),
            _ => {
                let px = (LETTER * item_d) as f32;
                let format =
                    text_format(&self.dwrite, "Segoe UI", DWRITE_FONT_WEIGHT_SEMI_BOLD, px)?;
                self.letter = Some((item_d.to_bits(), format.clone()));
                format
            }
        };
        // SAFETY: Direct2D and DirectWrite calls between BeginDraw and EndDraw.
        unsafe {
            let layout = self.layout(letter, &format)?;
            let metrics = metrics(&layout)?;
            let brush = if on {
                &self.brushes.ink_on
            } else {
                &self.brushes.ink
            };
            let origin = Vector2 {
                X: (size - metrics.width) / 2.0,
                Y: (size - metrics.height) / 2.0,
            };
            self.d2d.DrawTextLayout(
                origin,
                &layout,
                brush,
                NO_GLYPH_STYLE,
                0,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        Ok(())
    }

    /// A pill with the item name and what it does, outside the ring.
    unsafe fn draw_label(
        &self,
        draw: &LabelDraw,
        ring_center: Vec2,
        text: &LabelText,
    ) -> windows::core::Result<()> {
        let px = |at_100: f64| (at_100 * self.scale) as f32;
        // SAFETY: Direct2D and DirectWrite calls between BeginDraw and EndDraw.
        unsafe {
            let name = self.layout(&text.name, &self.fonts.name)?;
            let name_size = metrics(&name)?;
            let detail = match &text.detail {
                Some(detail) => {
                    let layout = self.layout(detail, &self.fonts.detail)?;
                    let size = metrics(&layout)?;
                    Some((layout, size))
                }
                None => None,
            };
            let (pad_x, pad_y, gap) = (px(12.0), px(6.0), px(8.0));
            let mut width = name_size.width + 2.0 * pad_x;
            let mut height = name_size.height;
            if let Some((_, size)) = &detail {
                width += gap + size.width;
                height = height.max(size.height);
            }
            height += 2.0 * pad_y;

            let bounds = Rect::new(0, 0, self.width as i32, self.height as i32);
            let top_left = label_position(
                draw,
                ring_center,
                (f64::from(width), f64::from(height)),
                f64::from(px(10.0)),
                bounds,
            );
            let (x, y) = (top_left.x as f32, top_left.y as f32);

            self.push_opacity(draw.opacity as f32)?;
            // Soft shadow under the pill: a few rounded rectangles, each a
            // little bigger and lower.
            for step in 1..=4 {
                let grow = px(2.0) * step as f32;
                let shadow = rounded(
                    x - grow,
                    y - grow + px(2.0),
                    width + 2.0 * grow,
                    height + 2.0 * grow,
                );
                self.d2d.FillRoundedRectangle(&shadow, &self.brushes.shadow);
            }
            self.d2d
                .FillRoundedRectangle(&rounded(x, y, width, height), &self.brushes.pill);
            let name_origin = Vector2 {
                X: x + pad_x,
                Y: y + pad_y,
            };
            self.d2d.DrawTextLayout(
                name_origin,
                &name,
                &self.brushes.name,
                NO_GLYPH_STYLE,
                0,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
            if let Some((layout, size)) = &detail {
                // Same baseline as the name.
                let shift = baseline(&name)? - baseline(layout)?;
                let origin = Vector2 {
                    X: name_origin.X + name_size.width + gap,
                    Y: name_origin.Y + shift,
                };
                let _ = size;
                self.d2d.DrawTextLayout(
                    origin,
                    layout,
                    &self.brushes.detail,
                    NO_GLYPH_STYLE,
                    0,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
            self.d2d.PopLayer();
        }
        Ok(())
    }

    unsafe fn push_opacity(&self, opacity: f32) -> windows::core::Result<()> {
        let params = D2D1_LAYER_PARAMETERS1 {
            contentBounds: D2D_RECT_F {
                left: f32::MIN,
                top: f32::MIN,
                right: f32::MAX,
                bottom: f32::MAX,
            },
            geometricMask: ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        // SAFETY: called between BeginDraw and EndDraw, paired with PopLayer.
        unsafe { self.d2d.PushLayer(&params, None) };
        Ok(())
    }

    unsafe fn layout(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
    ) -> windows::core::Result<IDWriteTextLayout> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: DirectWrite copies the text.
        unsafe {
            self.dwrite
                .CreateTextLayout(&wide, format, 10_000.0, 10_000.0)
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: the handle came from the swap chain and is closed once.
        let _ = unsafe { CloseHandle(self.frame_waiter) };
    }
}

/// Text that goes with a label target: the item name and its command.
pub fn label_text(ring: &Ring, target: Target) -> Option<LabelText> {
    let key = ItemKey::of(target)?;
    let action = match key {
        ItemKey::Slot(slot) => match ring.slots.get(usize::from(slot))? {
            Slot::Action(action) => action,
            _ => return None,
        },
        ItemKey::Child { slot, index } => match ring.slots.get(usize::from(slot))? {
            Slot::Folder { items, .. } => items.get(usize::from(index))?,
            _ => return None,
        },
    };
    Some(LabelText {
        name: action.name.clone(),
        detail: action.command.as_ref().map(ToString::to_string),
    })
}

/// Name and icon of an item.
fn item_of(ring: &Ring, key: ItemKey) -> Option<(&str, Option<&str>)> {
    match key {
        ItemKey::Slot(slot) => match ring.slots.get(usize::from(slot))? {
            Slot::Action(a) => Some((&a.name, a.icon.as_deref())),
            Slot::Folder { name, icon, .. } => Some((name, icon.as_deref())),
            Slot::Empty => None,
        },
        ItemKey::Child { slot, index } => match ring.slots.get(usize::from(slot))? {
            Slot::Folder { items, .. } => items
                .get(usize::from(index))
                .map(|a| (a.name.as_str(), a.icon.as_deref())),
            _ => None,
        },
    }
}

fn blob_constants(b: &Blob) -> ([f32; 4], [f32; 4]) {
    (
        [b.center.x as f32, b.center.y as f32, b.r as f32, 1.0],
        [
            b.dir.x as f32,
            b.dir.y as f32,
            b.stretch as f32,
            b.squash as f32,
        ],
    )
}

fn rounded(x: f32, y: f32, width: f32, height: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        },
        radiusX: height / 2.0,
        radiusY: height / 2.0,
    }
}

unsafe fn metrics(layout: &IDWriteTextLayout) -> windows::core::Result<DWRITE_TEXT_METRICS> {
    let mut metrics = DWRITE_TEXT_METRICS::default();
    // SAFETY: `metrics` is a local of the right type.
    unsafe { layout.GetMetrics(&mut metrics)? };
    Ok(metrics)
}

/// Distance from the top of the first line to its baseline.
unsafe fn baseline(layout: &IDWriteTextLayout) -> windows::core::Result<f32> {
    let mut lines = [DWRITE_LINE_METRICS::default(); 1];
    let mut count = 0;
    // SAFETY: room for one line; a longer text only reports the count.
    let _ = unsafe { layout.GetLineMetrics(Some(&mut lines), &mut count) };
    Ok(lines[0].baseline)
}

fn text_format(
    dwrite: &IDWriteFactory,
    family: &str,
    weight: DWRITE_FONT_WEIGHT,
    size: f32,
) -> windows::core::Result<IDWriteTextFormat> {
    // SAFETY: plain factory call with owned strings.
    unsafe {
        dwrite.CreateTextFormat(
            &HSTRING::from(family),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            &HSTRING::from(""),
        )
    }
}

/// A hardware device, or the software rasterizer if there is no GPU driver.
fn create_device() -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    let attempt = |driver: D3D_DRIVER_TYPE| {
        let mut device = None;
        let mut context = None;
        // SAFETY: out pointers are locals.
        unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map(|()| (device.expect("device"), context.expect("context")))
    };
    attempt(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| attempt(D3D_DRIVER_TYPE_WARP))
}

fn compile_shaders(
    device: &ID3D11Device,
) -> windows::core::Result<(ID3D11VertexShader, ID3D11PixelShader)> {
    let vertex = compile(s!("vs_main"), s!("vs_5_0"))?;
    let pixel = compile(s!("ps_main"), s!("ps_5_0"))?;
    let mut vs = None;
    let mut ps = None;
    // SAFETY: the bytecode slices come from blobs that live during the calls.
    unsafe {
        device.CreateVertexShader(bytes(&vertex), None, Some(&mut vs))?;
        device.CreatePixelShader(bytes(&pixel), None, Some(&mut ps))?;
    }
    Ok((vs.expect("vertex shader"), ps.expect("pixel shader")))
}

fn compile(entry: PCSTR, target: PCSTR) -> windows::core::Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    // SAFETY: the source is a static string; the blobs are out pointers.
    let result = unsafe {
        D3DCompile(
            SHADER.as_ptr() as *const _,
            SHADER.len(),
            s!("ring.hlsl"),
            None,
            None,
            entry,
            target,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(e) = result {
        let message = errors
            .map(|blob| String::from_utf8_lossy(bytes(&blob)).into_owned())
            .unwrap_or_default();
        return Err(windows::core::Error::new(e.code(), message));
    }
    Ok(code.expect("compiled shader"))
}

fn bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: the blob owns this many bytes for as long as it lives.
    unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
    }
}

fn create_constant_buffer(device: &ID3D11Device) -> windows::core::Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: size_of::<Constants>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut buffer = None;
    // SAFETY: no initial data; out pointer is a local.
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer))? };
    Ok(buffer.expect("constant buffer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_shader_layout() {
        // 32 + 24 circles and 12 vectors of 4 floats each.
        assert_eq!(size_of::<Constants>(), (32 + 24 + 12) * 16);
        assert_eq!(size_of::<Constants>() % 16, 0);
    }
}

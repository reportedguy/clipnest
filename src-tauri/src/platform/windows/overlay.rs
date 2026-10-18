//! Native region-selection overlay (Win32 + GDI).
//!
//! One borderless topmost window per monitor, each painting its monitor's frozen frame
//! dimmed, with the dragged rectangle shown undimmed plus a pixel-size label. The process
//! is per-monitor DPI aware (Tauri sets this), so window and mouse coordinates are
//! physical pixels: exactly the frozen frame's own coordinates.
//!
//! Runs entirely on the calling thread: windows are created, a message loop runs until
//! the user finishes, then everything is destroyed.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::Once;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, ExtCreatePen, GetDC, GetStockObject, InvalidateRect, LineTo, MoveToEx,
    Rectangle, ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, BS_SOLID, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CALCRECT,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_SWISS, FW_NORMAL, HBITMAP, HBRUSH, HDC, HFONT, HGDIOBJ, HPEN, LOGBRUSH,
    NULL_BRUSH, NULL_PEN, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_ENDCAP_ROUND, PS_GEOMETRIC, PS_JOIN_ROUND, PS_SOLID,
    PS_USERSTYLE, SRCCOPY, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW,
    LoadCursorW, PostQuitMessage, RegisterClassExW, SetCursor, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
    TranslateMessage, GWLP_USERDATA, IDC_CROSS, MSG, SW_SHOW, WM_CLOSE, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

use crate::capture::geometry::{self, PhysicalRect};
use crate::error::{Error, Result};
use crate::platform::{FrozenFrame, MonitorInfo, Region, RegionSelector};

const CLASS: PCWSTR = w!("ClipNestOverlay");
/// Also used by tests and tooling to find the overlay.
const TITLE: PCWSTR = w!("ClipNest capture");
const HINT: &str = "Drag to capture  ·  Esc to cancel";
/// Selection accent, the brand blue #0058FE (COLORREF is 0x00BBGGRR): the logo's finishing corner.
const ACCENT: COLORREF = COLORREF(0x00fe_5800);
/// The logo's dashed outline and top-left "+" on the dimmed screen.
const WHITE: COLORREF = COLORREF(0x00ff_ffff);
/// Neutral label background, DESIGN.md ink #181d26.
const INK: COLORREF = COLORREF(0x0026_1d18);
/// Length of each arm of the corner "+" marks, in DIPs.
const ARM: f32 = 10.0;
/// Brightness kept outside the selection (out of 256).
const DIM: u32 = 140;

pub struct GdiRegionSelector;

impl RegionSelector for GdiRegionSelector {
    fn select(&self, frames: &[FrozenFrame], on_shown: &mut dyn FnMut()) -> Result<Option<Region>> {
        if frames.is_empty() {
            return Err(Error::Capture("no monitors to capture".into()));
        }
        // SAFETY: all Win32 calls happen on this thread, on handles created here and
        // destroyed before returning.
        unsafe { run(frames, on_shown) }
    }
}

/// GDI resources for one monitor's window.
struct Surface {
    hwnd: HWND,
    width: i32,
    height: i32,
    dim: MemBitmap,
    original: MemBitmap,
    back: MemBitmap,
    font: HFONT,
    /// Scales layout constants (padding, offsets) by this monitor's DPI.
    dpi_scale: f32,
}

/// A bitmap selected into its own memory DC.
struct MemBitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl MemBitmap {
    unsafe fn free(&self) {
        SelectObject(self.dc, self.previous);
        let _ = DeleteObject(self.bitmap.into());
        let _ = DeleteDC(self.dc);
    }
}

struct Drag {
    surface: usize,
    start: (i32, i32),
    current: (i32, i32),
}

struct Overlay {
    surfaces: Vec<Surface>,
    drag: Option<Drag>,
    /// Set when the user finishes; ends the message loop.
    outcome: Option<Option<Region>>,
    accent: HBRUSH,
    ink: HBRUSH,
}

thread_local! {
    static STATE: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn with_state<T>(f: impl FnOnce(&mut Overlay) -> T) -> Option<T> {
    STATE.with(|s| s.borrow_mut().as_mut().map(f))
}

unsafe fn run(frames: &[FrozenFrame], on_shown: &mut dyn FnMut()) -> Result<Option<Region>> {
    register_class()?;
    let instance = GetModuleHandleW(None).map_err(win_err)?;
    let screen = GetDC(None);

    let mut surfaces = Vec::with_capacity(frames.len());
    let mut build = || -> Result<()> {
        for (i, frame) in frames.iter().enumerate() {
            let m = &frame.monitor;
            let (width, height) = (frame.image.width() as i32, frame.image.height() as i32);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                CLASS,
                TITLE,
                WS_POPUP,
                m.x,
                m.y,
                width,
                height,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(win_err)?;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (i + 1) as isize);

            let dpi_scale = GetDpiForWindow(hwnd) as f32 / 96.0;
            let raw = frame.image.as_raw();
            let original = dib(screen, width, height, |out| {
                for (o, px) in out.as_chunks_mut::<4>().0.iter_mut().zip(raw.as_chunks::<4>().0) {
                    *o = [px[2], px[1], px[0], 0xff];
                }
            })?;
            let dim = dib(screen, width, height, |out| {
                let d = |v: u8| ((u32::from(v) * DIM) >> 8) as u8;
                for (o, px) in out.as_chunks_mut::<4>().0.iter_mut().zip(raw.as_chunks::<4>().0) {
                    *o = [d(px[2]), d(px[1]), d(px[0]), 0xff];
                }
            })?;
            let back = compatible(screen, width, height)?;
            let font = CreateFontW(
                -((14.0 * dpi_scale).round() as i32),
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FF_SWISS.0 as u32,
                w!("Segoe UI"),
            );
            surfaces.push(Surface { hwnd, width, height, dim, original, back, font, dpi_scale });
        }
        Ok(())
    };
    let built = build();
    ReleaseDC(None, screen);

    STATE.with(|s| {
        *s.borrow_mut() = Some(Overlay {
            surfaces,
            drag: None,
            outcome: None,
            accent: CreateSolidBrush(ACCENT),
            ink: CreateSolidBrush(INK),
        })
    });
    if let Err(e) = built {
        teardown();
        return Err(e);
    }

    // Show everything, then give keyboard focus to the monitor under the pointer.
    let hwnds: Vec<HWND> = with_state(|s| s.surfaces.iter().map(|x| x.hwnd).collect()).unwrap_or_default();
    for &hwnd in &hwnds {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }
    let monitors: Vec<MonitorInfo> = frames.iter().map(|f| f.monitor.clone()).collect();
    let mut cursor = POINT::default();
    let focus =
        GetCursorPos(&mut cursor).ok().and_then(|_| geometry::monitor_at(&monitors, cursor.x, cursor.y)).unwrap_or(0);
    let _ = SetForegroundWindow(hwnds[focus]);
    let _ = SetFocus(Some(hwnds[focus]));
    on_shown();

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    let outcome = with_state(|s| s.outcome.take()).flatten().flatten();
    teardown();
    Ok(outcome)
}

unsafe fn teardown() {
    let Some(state) = STATE.with(|s| s.borrow_mut().take()) else { return };
    for s in &state.surfaces {
        let _ = DestroyWindow(s.hwnd);
        s.dim.free();
        s.original.free();
        s.back.free();
        let _ = DeleteObject(s.font.into());
    }
    let _ = DeleteObject(state.accent.into());
    let _ = DeleteObject(state.ink.into());
}

unsafe fn register_class() -> Result<()> {
    static REGISTER: Once = Once::new();
    let mut result = Ok(());
    REGISTER.call_once(|| {
        let registered = GetModuleHandleW(None).map(|instance| {
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassExW(&class)
        });
        if !matches!(registered, Ok(atom) if atom != 0) {
            result = Err(Error::Platform("couldn't register the capture overlay window class".into()));
        }
    });
    result
}

/// A top-down 32-bit DIB filled by `fill` (BGRA rows), selected into a new memory DC.
unsafe fn dib(screen: HDC, width: i32, height: i32, fill: impl FnOnce(&mut [u8])) -> Result<MemBitmap> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0).map_err(win_err)?;
    if bits.is_null() {
        let _ = DeleteObject(bitmap.into());
        return Err(Error::Capture("out of memory for the capture overlay".into()));
    }
    fill(std::slice::from_raw_parts_mut(bits.cast::<u8>(), (width * height * 4) as usize));
    let dc = CreateCompatibleDC(Some(screen));
    let previous = SelectObject(dc, bitmap.into());
    Ok(MemBitmap { dc, bitmap, previous })
}

unsafe fn compatible(screen: HDC, width: i32, height: i32) -> Result<MemBitmap> {
    let bitmap = CreateCompatibleBitmap(screen, width, height);
    if bitmap.is_invalid() {
        return Err(Error::Capture("out of memory for the capture overlay".into()));
    }
    let dc = CreateCompatibleDC(Some(screen));
    let previous = SelectObject(dc, bitmap.into());
    Ok(MemBitmap { dc, bitmap, previous })
}

fn win_err(e: windows::core::Error) -> Error {
    Error::Platform(format!("capture overlay: {}", e.message()))
}

/// Mouse position from a mouse message; signed because a captured pointer can leave the window.
fn point(lparam: LPARAM) -> (i32, i32) {
    let v = lparam.0 as u32;
    (i32::from(v as u16 as i16), i32::from((v >> 16) as u16 as i16))
}

/// Ends the message loop with a result.
unsafe fn finish(outcome: Option<Region>) {
    with_state(|s| s.outcome = Some(outcome));
    PostQuitMessage(0);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let tagged = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if tagged <= 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let index = (tagged - 1) as usize;

    match msg {
        WM_ERASEBKGND => LRESULT(1), // everything is painted in WM_PAINT; avoids flicker
        WM_SETCURSOR => {
            if let Ok(cursor) = LoadCursorW(None, IDC_CROSS) {
                SetCursor(Some(cursor));
            }
            LRESULT(1)
        }
        WM_PAINT => {
            paint(hwnd, index);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let p = point(lparam);
            with_state(|s| s.drag = Some(Drag { surface: index, start: p, current: p }));
            SetCapture(hwnd);
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let p = point(lparam);
            let moved = with_state(|s| match s.drag.as_mut() {
                Some(d) if d.surface == index => {
                    d.current = p;
                    true
                }
                _ => false,
            });
            if moved == Some(true) {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // Use the release point itself; the last WM_MOUSEMOVE can lag a fast drag.
            let p = point(lparam);
            let region = with_state(|s| {
                let d = s.drag.take()?;
                let sf = &s.surfaces[d.surface];
                geometry::from_drag(d.start, p, sf.width as u32, sf.height as u32)
                    .map(|rect| Region { frame: d.surface, rect })
            })
            .flatten();
            // Outside the state borrow: ReleaseCapture sends WM_CAPTURECHANGED synchronously.
            let _ = ReleaseCapture();
            match region {
                Some(r) => finish(Some(r)),
                None => {
                    let _ = InvalidateRect(Some(hwnd), None, false); // a click: clear and keep going
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP | WM_CLOSE => {
            finish(None);
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => {
            finish(None);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint(hwnd: HWND, index: usize) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    with_state(|s| {
        let sf = &s.surfaces[index];
        let back = sf.back.dc;
        let _ = BitBlt(back, 0, 0, sf.width, sf.height, Some(sf.dim.dc), 0, 0, SRCCOPY);

        let selection = s.drag.as_ref().filter(|d| d.surface == index).map(|d| {
            // Same clamping as the final crop, but allow tiny sizes while dragging.
            let c = |v: i32, max: i32| v.clamp(0, max);
            let (x0, x1) = (c(d.start.0.min(d.current.0), sf.width), c(d.start.0.max(d.current.0), sf.width));
            let (y0, y1) = (c(d.start.1.min(d.current.1), sf.height), c(d.start.1.max(d.current.1), sf.height));
            PhysicalRect { x: x0 as u32, y: y0 as u32, width: (x1 - x0) as u32, height: (y1 - y0) as u32 }
        });

        let old_font = SelectObject(back, sf.font.into());
        SetBkMode(back, TRANSPARENT);
        SetTextColor(back, COLORREF(0x00ff_ffff));
        let pad = (5.0 * sf.dpi_scale).round() as i32;

        match selection {
            Some(r) => {
                let (x, y, w, h) = (r.x as i32, r.y as i32, r.width as i32, r.height as i32);
                let _ = BitBlt(back, x, y, w, h, Some(sf.original.dc), x, y, SRCCOPY);
                selection_frame(back, x, y, w, h, sf.dpi_scale);
            }
            None => {
                let mut text: Vec<u16> = HINT.encode_utf16().collect();
                let mut size = RECT::default();
                DrawTextW(back, &mut text, &mut size, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
                let (lw, lh) = (size.right + 4 * pad, size.bottom + 2 * pad);
                let top = (24.0 * sf.dpi_scale).round() as i32;
                label(back, s.ink, &mut text, (sf.width - lw) / 2, top, lw, lh, 2 * pad, sf.dpi_scale);
            }
        }
        SelectObject(back, old_font);
        let _ = BitBlt(hdc, 0, 0, sf.width, sf.height, Some(back), 0, 0, SRCCOPY);
    });
    let _ = EndPaint(hwnd, &ps);
}

/// The selection outline, drawn like the logo: a white dashed border with rounded dashes,
/// a white "+" on the top-left corner and a brand-blue "+" on the bottom-right one.
unsafe fn selection_frame(dc: HDC, x: i32, y: i32, w: i32, h: i32, dpi_scale: f32) {
    let px = |v: f32| ((v * dpi_scale).round() as i32).max(1);
    let line = px(2.0) as u32;
    let pen = |color: COLORREF, width: u32, dash: Option<&[u32]>| -> HPEN {
        let brush = LOGBRUSH { lbStyle: BS_SOLID, lbColor: color, lbHatch: 0 };
        let style = if dash.is_some() { PS_USERSTYLE } else { PS_SOLID };
        ExtCreatePen(PS_GEOMETRIC | style | PS_ENDCAP_ROUND | PS_JOIN_ROUND, width, &brush, dash)
    };

    // Dashed border just outside the selection so it never covers captured pixels. Round
    // caps add half a line to each end, so the gaps are drawn longer than they look.
    let dashed = pen(WHITE, line, Some(&[line * 3, line * 3]));
    let old_pen = SelectObject(dc, dashed.into());
    let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    let o = line as i32 / 2 + 1;
    let (l, t, r, b) = (x - o, y - o, x + w + o, y + h + o);
    let _ = Rectangle(dc, l, t, r, b);

    // Corner crosses, a little bolder than the border, like the logo's. The blue one's
    // upper arm runs further up the edge, as in the logo.
    let arm = px(ARM);
    let bold = px(3.0) as u32;
    let white = pen(WHITE, bold, None);
    let blue = pen(ACCENT, bold, None);
    for (cx, cy, p, up) in [(l, t, white, arm), (r, b, blue, (2 * arm).min(h).max(arm))] {
        SelectObject(dc, p.into());
        let _ = MoveToEx(dc, cx - arm, cy, None);
        let _ = LineTo(dc, cx + arm, cy);
        let _ = MoveToEx(dc, cx, cy - up, None);
        let _ = LineTo(dc, cx, cy + arm);
    }

    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    for p in [dashed, white, blue] {
        let _ = DeleteObject(p.into());
    }
}

/// A rounded pill with white text.
#[allow(clippy::too_many_arguments)]
unsafe fn label(dc: HDC, bg: HBRUSH, text: &mut [u16], x: i32, y: i32, w: i32, h: i32, pad_x: i32, dpi_scale: f32) {
    let radius = ((8.0 * dpi_scale).round() as i32).max(2);
    let old_brush = SelectObject(dc, bg.into());
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    let _ = RoundRect(dc, x, y, x + w + 1, y + h + 1, radius, radius);
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    let mut rect = RECT { left: x + pad_x, top: y, right: x + w, bottom: y + h };
    DrawTextW(dc, text, &mut rect, DT_SINGLELINE | DT_NOPREFIX | DT_VCENTER);
}

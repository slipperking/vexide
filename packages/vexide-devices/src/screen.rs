//! Brain screen display and touch functions.
//!
//! Contains user calls to the V5 Brain screen for touching and displaying graphics.
//! The [`Fill`] trait can be used to draw filled in shapes to the screen
//! and the [`Stroke`] trait can be used to draw the outlines of shapes.

use alloc::{ffi::CString, string::String, vec::Vec};
use core::{mem, time::Duration};

use snafu::Snafu;
use vex_sdk::{
    vexDisplayBackgroundColor, vexDisplayBigStringAt, vexDisplayCircleDraw, vexDisplayCircleFill,
    vexDisplayCopyRect, vexDisplayErase, vexDisplayForegroundColor, vexDisplayLineDraw,
    vexDisplayPixelSet, vexDisplayRectDraw, vexDisplayRectFill, vexDisplayScroll,
    vexDisplayScrollRect, vexDisplaySmallStringAt, vexDisplayString, vexDisplayStringAt,
    vexDisplayStringHeightGet, vexDisplayStringWidthGet, vexTouchDataGet, V5_TouchEvent,
    V5_TouchStatus,
};

use crate::{color::IntoRgb, geometry::Point2};

/// Represents the physical display on the V5 Brain.
#[derive(Debug, Eq, PartialEq)]
pub struct Screen {
    writer_buffer: String,
    render_mode: RenderMode,
    current_line: usize,
}

impl core::fmt::Write for Screen {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for character in text.chars() {
            if character == '\n' {
                if self.current_line > (Self::MAX_VISIBLE_LINES - 2) {
                    self.scroll(0, Self::LINE_HEIGHT);
                    self.flush_writer();
                } else {
                    self.flush_writer();
                    self.current_line += 1;
                }
            } else {
                self.writer_buffer.push(character);
            }
        }

        unsafe {
            vexDisplayForegroundColor(0xffffff);
            vexDisplayString(
                self.current_line as i32,
                c"%s".as_ptr(),
                CString::new(self.writer_buffer.clone())
                    .expect(
                        "CString::new encountered NUL (U+0000) byte in non-terminating position.",
                    )
                    .into_raw(),
            );
        }

        Ok(())
    }
}

/// A type implementing this trait can draw a filled shape to the display.
pub trait Fill {
    /// Draw a filled shape to the display.
    fn fill(&self, screen: &mut Screen, color: impl IntoRgb);
}

/// A type implementing this trait can draw an outlined shape to the display.
pub trait Stroke {
    /// Draw an outlined shape to the display.
    fn stroke(&self, screen: &mut Screen, color: impl IntoRgb);
}

/// A circle that can be drawn on the screen
///
/// Circles are not antialiased.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Circle {
    /// Center point of the circle
    pub center: Point2<i16>,

    /// Radius of the circle
    pub radius: u16,
}

impl Circle {
    /// Create a circle with the given coordinates and radius.
    /// The coordinates are the center of the circle.
    pub fn new(center: impl Into<Point2<i16>>, radius: u16) -> Self {
        Self {
            center: center.into(),
            radius,
        }
    }
}

impl Fill for Circle {
    fn fill(&self, _screen: &mut Screen, color: impl IntoRgb) {
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayCircleFill(
                self.center.x as _,
                (self.center.y + Screen::HEADER_HEIGHT) as _,
                self.radius as i32,
            );
        }
    }
}

impl Stroke for Circle {
    fn stroke(&self, _screen: &mut Screen, color: impl IntoRgb) {
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayCircleDraw(
                self.center.x as _,
                (self.center.y + Screen::HEADER_HEIGHT) as _,
                self.radius as i32,
            );
        }
    }
}

/// A line that can be drawn on the screen.
/// The width is the same as the pen width.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Line {
    /// Start point (coordinate) of the line
    pub start: Point2<i16>,

    /// End point (coordinate) of the line
    pub end: Point2<i16>,
}

impl Line {
    /// Create a new line with a given start and end coordinate.
    pub fn new(start: impl Into<Point2<i16>>, end: impl Into<Point2<i16>>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

impl Fill for Line {
    fn fill(&self, _screen: &mut Screen, color: impl IntoRgb) {
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayLineDraw(
                self.start.x as _,
                (self.start.y + Screen::HEADER_HEIGHT) as _,
                self.end.x as _,
                (self.end.y + Screen::HEADER_HEIGHT) as _,
            );
        }
    }
}

impl<T: Into<Point2<i16>> + Copy> Fill for T {
    fn fill(&self, _screen: &mut Screen, color: impl IntoRgb) {
        let point: Point2<i16> = (*self).into();

        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayPixelSet(point.x as _, (point.y + Screen::HEADER_HEIGHT) as _);
        }
    }
}

/// A rectangular region of the screen
///
/// When drawn to the display, both the start and the end points are included inside
/// the drawn region. Thus, the area of the drawn rectangle is
/// `(1 + end.x - start.x) * (1 + end.y - start.y)` pixels.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Rect {
    /// First point (coordinate) of the rectangle
    pub start: Point2<i16>,

    /// Second point (coordinate) of the rectangle
    pub end: Point2<i16>,
}

impl Rect {
    /// Create a new rectangle with the given coordinates.
    pub fn new(start: impl Into<Point2<i16>>, end: impl Into<Point2<i16>>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }

    /// Create a new rectangle from a given origin point (top-left) and dimensions (width/height).
    pub fn from_dimensions(origin: impl Into<Point2<i16>>, width: u16, height: u16) -> Self {
        let origin = origin.into();
        Self {
            start: origin,
            end: Point2 {
                x: origin.x + (width as i16),
                y: origin.y + (height as i16),
            },
        }
    }

    /// Create a new rectangle from a given origin point (top-left) and dimensions (width/height).
    pub fn from_dimensions_centered(
        center: impl Into<Point2<i16>>,
        width: u16,
        height: u16,
    ) -> Self {
        let center = center.into();

        Self::from_dimensions(
            Point2 {
                x: center.x - (width as i16) / 2,
                y: center.y - (height as i16) / 2,
            },
            width,
            height,
        )
    }
}

impl Stroke for Rect {
    fn stroke(&self, _screen: &mut Screen, color: impl IntoRgb) {
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayRectDraw(
                self.start.x as _,
                (self.start.y + Screen::HEADER_HEIGHT) as _,
                self.end.x as _,
                (self.end.y + Screen::HEADER_HEIGHT) as _,
            );
        }
    }
}

impl Fill for Rect {
    fn fill(&self, _screen: &mut Screen, color: impl IntoRgb) {
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());
            vexDisplayRectFill(
                self.start.x as _,
                (self.start.y + Screen::HEADER_HEIGHT) as _,
                self.end.x as _,
                (self.end.y + Screen::HEADER_HEIGHT) as _,
            );
        }
    }
}

/// Options for how a text object should be formatted.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TextSize {
    /// Small text.
    Small,
    /// Medium text.
    Medium,
    /// Large text.
    Large,
}

/// Horizontal alignment for text on the screen
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum HAlign {
    /// Input coordinate is at the left of the text box
    #[default]
    Left,
    /// Input coordinate is at the center of the text box
    Center,
    /// Input coordinate is at the right of the text box
    Right,
}

/// Vertical alignment for text on the screen
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum VAlign {
    /// Input coordinate is at the top of the text box
    #[default]
    Top,
    /// Input coordinate is at the center of the text box
    Center,
    /// Input coordinate is at the bottom of the text box
    Bottom,
}

/// A piece of text that can be drawn on the display.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Text {
    /// Top left corner coordinates of text on the screen
    pub position: Point2<i16>,
    /// C-String of the desired text to be displayed on the screen
    pub text: CString,
    /// Size of text to be displayed on the screen
    pub size: TextSize,
    /// Horizontal alignment of text displayed on the screen
    pub horizontal_align: HAlign,
    /// Vertical alignment of text displayed on the screen
    pub vertical_align: VAlign,
}

impl Text {
    /// Create a new text with a given position (defaults to top left corner alignment) and format
    pub fn new(text: &str, size: TextSize, position: impl Into<Point2<i16>>) -> Self {
        Self::new_aligned(text, size, position, HAlign::default(), VAlign::default())
    }

    /// Create a new text with a given position (based on alignment) and format
    pub fn new_aligned(
        text: &str,
        size: TextSize,
        position: impl Into<Point2<i16>>,
        horizontal_align: HAlign,
        vertical_align: VAlign,
    ) -> Self {
        Self {
            text: CString::new(text)
                .expect("CString::new encountered NUL (U+0000) byte in non-terminating position."),
            position: position.into(),
            size,
            horizontal_align,
            vertical_align,
        }
    }

    /// Change text alignment
    pub fn align(&mut self, horizontal_align: HAlign, vertical_align: VAlign) {
        self.horizontal_align = horizontal_align;
        self.vertical_align = vertical_align;
    }

    /// Get the height of the text widget in pixels
    pub fn height(&self) -> u16 {
        unsafe {
            // Display blank string(no-op function) to set last used text size
            // vexDisplayString(Height/Width)Get uses the last text size to determine text size
            match self.size {
                TextSize::Small => {
                    vexDisplaySmallStringAt(0, 0, c"".as_ptr());
                }
                TextSize::Medium => {
                    vexDisplayStringAt(0, 0, c"".as_ptr());
                }
                TextSize::Large => {
                    vexDisplayBigStringAt(0, 0, c"".as_ptr());
                }
            }

            vexDisplayStringHeightGet(self.text.as_ptr()) as _
        }
    }

    /// Get the width of the text widget in pixels
    pub fn width(&self) -> u16 {
        unsafe {
            match self.size {
                // Display blank string(no-op function) to set last used text size
                // vexDisplayString(Height/Width)Get uses the last text size to determine text size
                TextSize::Small => {
                    vexDisplaySmallStringAt(0, 0, c"".as_ptr());
                }
                TextSize::Medium => {
                    vexDisplayStringAt(0, 0, c"".as_ptr());
                }
                TextSize::Large => {
                    vexDisplayBigStringAt(0, 0, c"".as_ptr());
                }
            }

            vexDisplayStringWidthGet(self.text.as_ptr()) as _
        }
    }
}

impl Fill for Text {
    fn fill(&self, _screen: &mut Screen, color: impl IntoRgb) {
        // Horizontally align text
        let x = match self.horizontal_align {
            HAlign::Left => self.position.x,
            HAlign::Center => self.position.x - (self.width() / 2) as i16,
            HAlign::Right => self.position.x - self.width() as i16,
        };

        // Vertically align text
        let y = match self.vertical_align {
            VAlign::Top => self.position.y,
            VAlign::Center => self.position.y - (self.height() / 2) as i16,
            VAlign::Bottom => self.position.y - self.height() as i16,
        };

        // This implementation is technically broken because it doesn't account errno.
        // This will be fixed once we have switched to vex-sdk.
        unsafe {
            vexDisplayForegroundColor(color.into_rgb().into());

            // Use `%s` and varargs to escape the string to stop undefined and unsafe behavior
            match self.size {
                TextSize::Small => vexDisplaySmallStringAt(
                    x as _,
                    (y + Screen::HEADER_HEIGHT) as _,
                    c"%s".as_ptr(),
                    self.text.as_ptr(),
                ),
                TextSize::Medium => vexDisplayStringAt(
                    x as _,
                    (y + Screen::HEADER_HEIGHT) as _,
                    c"%s".as_ptr(),
                    self.text.as_ptr(),
                ),
                TextSize::Large => vexDisplayBigStringAt(
                    x as _,
                    (y + Screen::HEADER_HEIGHT) as _,
                    c"%s".as_ptr(),
                    self.text.as_ptr(),
                ),
            }
        }
    }
}

/// A touch event on the screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TouchEvent {
    /// Touch state.
    pub state: TouchState,
    /// X coordinate of the touch.
    pub x: i16,
    /// Y coordinate of the touch.
    pub y: i16,
    /// how many times the screen has been pressed.
    pub press_count: i32,
    /// how many times the screen has been released.
    pub release_count: i32,
}

/// The state of a given touch.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TouchState {
    /// The touch has been released.
    Released,
    /// The screen has been touched.
    Pressed,
    /// The screen has been touched and is still being held.
    Held,
}

impl From<V5_TouchEvent> for TouchState {
    fn from(value: V5_TouchEvent) -> Self {
        match value {
            V5_TouchEvent::kTouchEventPress => Self::Pressed,
            V5_TouchEvent::kTouchEventRelease => Self::Released,
            V5_TouchEvent::kTouchEventPressAuto => Self::Held,
            _ => unreachable!(),
        }
    }
}

/// The rendering mode for the VEX V5's display
///
/// When using the screen in the [`Immediate`](RenderMode::Immediate) mode, all draw operations will immediately show up on the display.
/// The [`DoubleBuffered`](RenderMode::DoubleBuffered) mode instead applies draw operations onto an intermediate buffer
/// that can be swapped onto the screen by calling [`Screen::render`], thereby preventing screen tearing.
/// By default, the screen uses the [`Immediate`](RenderMode::Immediate) mode.
/// # Note
/// [`Screen::render`] **MUST** be called for anything to appear on the screen when using the [`DoubleBuffered`](RenderMode::DoubleBuffered) mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    /// Draw operations are immediately applied to the screen without the need to call [`Screen::render`].
    Immediate,
    /// Draw calls are affected on an intermediary display buffer, rather than directly drawn to the screen.
    /// The intermediate buffer can later be applied to the screen using [`Screen::render`].
    ///
    /// This mode is necessary for preventing screen tearing when drawing at high speeds.
    DoubleBuffered,
}

impl Screen {
    /// The maximum number of lines that can be visible on the screen at once.
    pub(crate) const MAX_VISIBLE_LINES: usize = 12;

    /// The height of a single line of text on the screen.
    pub(crate) const LINE_HEIGHT: i16 = 20;

    /// Vertical height taken by the user program header when visible.
    pub const HEADER_HEIGHT: i16 = 32;

    /// The horizontal resolution of the display.
    #[cfg(not(target_env = "exp"))]
    pub const HORIZONTAL_RESOLUTION: i16 = 480;

    /// The vertical resolution of the writable part of the display.
    #[cfg(not(target_env = "exp"))]
    pub const VERTICAL_RESOLUTION: i16 = 240;

    /// The horizontal resolution of the display.
    #[cfg(target_env = "exp")]
    pub const HORIZONTAL_RESOLUTION: i16 = 160;

    /// The vertical resolution of the writable part of the display.
    #[cfg(target_env = "exp")]
    pub const VERTICAL_RESOLUTION: i16 = 108;

    /// The amount of time it takes for the brain display to fully re-render.
    /// The brain display is 60fps.
    pub const REFRESH_INTERVAL: Duration = Duration::from_micros(16667);

    /// Create a new screen.
    ///
    /// # Safety
    ///
    /// Creating new `Screen`s is inherently unsafe due to the possibility of constructing
    /// more than one screen at once allowing multiple mutable references to the same
    /// hardware device. Prefer using [`Peripherals`](crate::peripherals::Peripherals) to register devices if possible.
    pub unsafe fn new() -> Self {
        Self {
            current_line: 0,
            render_mode: RenderMode::Immediate,
            writer_buffer: String::default(),
        }
    }

    fn flush_writer(&mut self) {
        unsafe {
            vexDisplayForegroundColor(0xffffff);
            vexDisplayString(
                self.current_line as i32,
                c"%s".as_ptr(),
                CString::new(self.writer_buffer.clone())
                    .expect(
                        "CString::new encountered NUL (U+0000) byte in non-terminating position.",
                    )
                    .into_raw(),
            );
        }

        self.writer_buffer.clear();
    }

    /// Set the render mode for the screen.
    /// For more info on render modes, look at the [`RenderMode`] docs.
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.render_mode = mode;
        unsafe {
            match mode {
                RenderMode::Immediate => vex_sdk::vexDisplayDoubleBufferDisable(),
                RenderMode::DoubleBuffered => vex_sdk::vexDisplayRender(false, true),
            }
        }
    }

    /// Gets the [`RenderMode`] of the screen.
    pub const fn render_mode(&self) -> RenderMode {
        self.render_mode
    }

    /// Flushes the screens double buffer if it is enabled.
    /// This is a no-op with the [`Immediate`](RenderMode::Immediate) rendering mode,
    /// but is necessary for anything to be displayed on the screen when using the [`DoubleBuffered`](RenderMode::DoubleBuffered) mode.
    pub fn render(&mut self) {
        if let RenderMode::DoubleBuffered = self.render_mode {
            unsafe {
                // TODO: create an async function that does the equivalent of `bVsyncWait`.
                vex_sdk::vexDisplayRender(false, false)
            }
        }
    }

    /// Scroll the pixels at or below the specified y-coordinate.
    ///
    /// This function y-offsets the pixels in the display buffer which are at or below the given start point (`start`) by
    /// a number (`offset`) of pixels. Positive values move the pixels upwards, and pixels that are moved out of the scroll
    /// region are discarded. Empty spaces are then filled with the display's background color.
    pub fn scroll(&mut self, start: i16, offset: i16) {
        unsafe { vexDisplayScroll(start.into(), offset.into()) }
    }

    /// Scroll a region of the screen.
    ///
    /// This function y-offsets the pixels in the display buffer which are contained in the specified scroll region (`region`) by
    /// a number (`offset`) of pixels. Positive values move the pixels upwards, and pixels that are moved out of the scroll
    /// region are discarded. Empty spaces are then filled with the display's background color.
    pub fn scroll_region(&mut self, region: Rect, offset: i16) {
        unsafe {
            vexDisplayScrollRect(
                region.start.x as _,
                (region.start.y + Self::HEADER_HEIGHT) as _,
                (region.end.x).into(),
                (region.end.y + Self::HEADER_HEIGHT) as _,
                offset as _,
            )
        }
    }

    /// Draw a filled object to the screen.
    pub fn fill(&mut self, shape: &impl Fill, color: impl IntoRgb) {
        shape.fill(self, color)
    }

    /// Draw an outlined object to the screen.
    pub fn stroke(&mut self, shape: &impl Stroke, color: impl IntoRgb) {
        shape.stroke(self, color)
    }

    /// Wipe the entire display buffer, filling it with a specified color.
    pub fn erase(&mut self, color: impl IntoRgb) {
        unsafe {
            vexDisplayBackgroundColor(color.into_rgb().into());
            vexDisplayErase();
        };
    }

    /// Draw a buffer of pixels to a specified region of the screen.
    ///
    /// This function copies the pixels in the specified buffer to the specified region of the screen.
    /// The stride parameter is defined as the number of pixels per row.
    pub fn draw_buffer<T, I>(
        &mut self,
        region: Rect,
        buf: T,
        src_stride: i32,
    ) -> Result<(), ScreenError>
    where
        T: IntoIterator<Item = I>,
        I: IntoRgb,
    {
        let mut raw_buf = buf
            .into_iter()
            .map(|i| i.into_rgb().into())
            .collect::<Vec<_>>();
        // Convert the coordinates to u32 to avoid overflows when multiplying.
        let expected_size = ((region.end.x - region.start.x) as u32
            * (region.end.y - region.start.y) as u32) as usize;
        if raw_buf.len() != expected_size {
            return Err(ScreenError::BufferSize {
                buffer_size: raw_buf.len(),
                expected_size,
            });
        }

        // SAFETY: The buffer is guaranteed to be the correct size.
        unsafe {
            vexDisplayCopyRect(
                region.start.x as _,
                (region.start.y + Self::HEADER_HEIGHT) as _,
                region.end.x as _,
                (region.end.y + Self::HEADER_HEIGHT) as _,
                raw_buf.as_mut_ptr(),
                src_stride,
            );
        }

        Ok(())
    }

    /// Get the current touch status of the screen.
    pub fn touch_status(&self) -> TouchEvent {
        // `vexTouchDataGet` (probably) doesn't read from the given status pointer, so this is fine.
        let mut touch_status: V5_TouchStatus = unsafe { mem::zeroed() };

        unsafe {
            vexTouchDataGet(&mut touch_status as *mut _);
        }

        TouchEvent {
            state: touch_status.lastEvent.into(),
            x: touch_status.lastXpos,
            y: touch_status.lastYpos,
            press_count: touch_status.pressCount,
            release_count: touch_status.releaseCount,
        }
    }
}

#[derive(Debug, Snafu)]
/// Errors that can occur when interacting with the screen.
pub enum ScreenError {
    /// The given buffer of colors was wrong size to fill the specified area.
    BufferSize {
        /// The size of the buffer.
        buffer_size: usize,
        /// The expected size of the buffer.
        expected_size: usize,
    },
}

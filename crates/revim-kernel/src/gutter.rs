//! Gutter: columns shown left of the text (line numbers, later git signs, diagnostics).
//!
//! Modules add columns with [`GutterProvider`]s registered through
//! [`Registrar::gutter`](crate::module::Registrar::gutter). Each frame the kernel asks every
//! provider for its column ([`Editor::gutter_columns`](crate::editor::Editor::gutter_columns))
//! and hands the result to the UI as plain data ([`GutterColumn`]); the UI only draws it.

use crate::{document::Document, window::Window};

/// What a gutter column shows next to one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterCell {
    /// Text of the cell. Should fit in the column's width; the UI cuts off the rest.
    pub text: String,
    /// How the cell looks; the UI maps it to a style.
    pub highlight: Highlight,
}

/// How a gutter cell looks. The UI maps each variant to a style.
///
/// New variants come with new gutter features (git, diagnostics): always match it with a
/// `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Highlight {
    /// Default text style.
    Normal,
    /// A line number.
    LineNumber,
    /// The number of the line with the cursor.
    CurrentLineNumber,
    /// Less important content, drawn dimmed so it does not compete with the text, e.g. a
    /// separator.
    Dimmed,
}

/// What a [`GutterProvider`] can look at: the window being drawn and its document.
#[non_exhaustive]
pub struct GutterContext<'a> {
    /// The document shown in the window.
    pub doc: &'a Document,
    /// The window being drawn.
    pub window: &'a Window,
}

/// One gutter column, added by a module with
/// [`Registrar::gutter`](crate::module::Registrar::gutter).
///
/// Called while drawing a frame: [`width`](Self::width), then [`cell`](Self::cell) for every
/// visible line. Keep both cheap. Columns are drawn left to right in the order they were
/// registered.
///
/// Calls are guarded like other module code: a provider that panics disables its module and
/// its column disappears.
///
/// ```
/// use revim_kernel::gutter::{GutterCell, GutterContext, GutterProvider, Highlight};
///
/// /// Marks every tenth line with `*`.
/// struct EveryTenth;
///
/// impl GutterProvider for EveryTenth {
///     fn width(&self, _cx: &GutterContext) -> u16 {
///         1
///     }
///     fn cell(&self, _cx: &GutterContext, line: usize) -> GutterCell {
///         let text = if (line + 1) % 10 == 0 { "*" } else { " " };
///         GutterCell { text: text.into(), highlight: Highlight::Normal }
///     }
/// }
/// ```
pub trait GutterProvider: 'static {
    /// Width of the column in screen cells. May depend on the context, e.g. line numbers
    /// get wider as the document grows. Return 0 to hide the column for this frame; `cell`
    /// is then not called.
    fn width(&self, cx: &GutterContext) -> u16;
    /// The cell for `line` (0-based). Called only for lines that exist in the document.
    fn cell(&self, cx: &GutterContext, line: usize) -> GutterCell;
}

/// A column ready to draw: one cell per visible line, from the window's scroll position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterColumn {
    /// Width in screen cells.
    pub width: u16,
    /// Cells of the visible lines, top to bottom; none for rows past the end of the document.
    pub cells: Vec<GutterCell>,
}

/// All registered providers with the id of the module that registered each, in order.
#[derive(Default)]
pub(crate) struct GutterProviderRegistry(Vec<(&'static str, Box<dyn GutterProvider>)>);

impl GutterProviderRegistry {
    pub(crate) fn push(&mut self, module: &'static str, provider: Box<dyn GutterProvider>) {
        self.0.push((module, provider))
    }

    /// Providers with their module ids, in registration order.
    pub(crate) fn providers(
        &self,
    ) -> impl Iterator<Item = (&'static str, &dyn GutterProvider)> + '_ {
        self.0.iter().map(|(m, p)| (*m, p.as_ref()))
    }
}

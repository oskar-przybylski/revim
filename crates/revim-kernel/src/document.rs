//! A single document: its text, address and version.

use std::path::PathBuf;

use ropey::Rope;

/// Address of a document: where its text comes from and where it is saved.
///
/// ```
/// use revim_kernel::document::Uri;
///
/// let uri = Uri::File("src/main.rs".into());
/// assert_eq!(uri, Uri::File("src/main.rs".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uri {
    /// A file on the local file system.
    File(PathBuf),
}

/// The text of one buffer together with where it belongs.
///
/// A document either has a [`Uri`] (it belongs to a file, possibly one that does not exist
/// yet) or none — a *scratch* buffer not tied to any file.
///
/// Every document has a [`version`](Self::version) starting at 0.
#[derive(Debug)]
pub struct Document {
    text: Rope,
    uri: Option<Uri>, // None means scratchpad
    version: u64,
}

impl Document {
    /// Creates an empty document not tied to any file.
    ///
    /// ```
    /// use revim_kernel::document::Document;
    ///
    /// let doc = Document::scratch();
    /// assert_eq!(doc.text().len_chars(), 0);
    /// assert!(doc.uri().is_none());
    /// assert_eq!(doc.version(), 0);
    /// ```
    pub fn scratch() -> Self {
        Self {
            text: Rope::new(),
            uri: None,
            version: 0,
        }
    }
    /// Creates a document with the given text and address, at version 0.
    ///
    /// Pass `Some(uri)` with an empty `text` for a file that does not exist yet (it will be
    /// created on save), `None` for a scratch buffer with initial content.
    ///
    /// ```
    /// use revim_kernel::document::{Document, Uri};
    ///
    /// let doc = Document::from_text(Some(Uri::File("a.rs".into())), "fn main() {}\n");
    /// assert_eq!(doc.text().to_string(), "fn main() {}\n");
    /// assert_eq!(doc.uri(), Some(&Uri::File("a.rs".into())));
    ///
    /// // a new, not yet existing file
    /// let new = Document::from_text(Some(Uri::File("new.rs".into())), "");
    /// assert_eq!(new.text().len_chars(), 0);
    /// ```
    pub fn from_text(uri: Option<Uri>, text: &str) -> Self {
        Self {
            text: Rope::from_str(text),
            uri,
            version: 0,
        }
    }
    /// The text of the document.
    pub fn text(&self) -> &Rope {
        &self.text
    }
    /// Where the document belongs, or `None` for a scratch buffer.
    pub fn uri(&self) -> Option<&Uri> {
        self.uri.as_ref()
    }
    /// Version of the text; 0 for a freshly created document.
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_text_keeps_content_and_uri() {
        let doc = Document::from_text(Some(Uri::File("a.rs".into())), "fn main() {}\n");
        assert_eq!(doc.text().to_string(), "fn main() {}\n");
        assert_eq!(doc.uri(), Some(&Uri::File("a.rs".into())));
        assert_eq!(doc.version(), 0);
    }

    #[test]
    fn scratch_is_empty_without_uri() {
        let doc = Document::scratch();
        assert_eq!(doc.text().len_chars(), 0);
        assert!(doc.uri().is_none());
    }
}

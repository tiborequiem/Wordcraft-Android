//! Scoped Storage (Storage Access Framework) & File I/O Bridge.
//!
//! Bridges Android SAF URIs / content streams directly into WordCraft engine I/O:
//! - `.docx`, `.docm`, `.odt`, `.rtf`, `.md`, `.txt`, `.html`, `.json` via `wordcraft_engine::io`
//! - `.pdf` export via `wordcraft_pdf::export`

use wordcraft_doc::Document;
use wordcraft_engine::io::{open_bytes_in, save_bytes_with, ExportOptions};
use wordcraft_geom::Region;

/// Opens document bytes loaded from an Android SAF content stream.
pub fn open_document_bytes(
    filename: &str,
    bytes: &[u8],
    password: Option<&str>,
    region: &Region,
) -> Result<Document, String> {
    let (doc, _encrypted) = open_bytes_in(filename, bytes, password, region)?;
    Ok(doc)
}

/// Serializes document into bytes suitable for writing back to an Android SAF OutputStream.
pub fn save_document_bytes(
    filename: &str,
    doc: &Document,
    password: Option<&str>,
    opts: Option<&ExportOptions>,
) -> Result<Vec<u8>, String> {
    let default_opts = ExportOptions::default();
    let export_opts = opts.unwrap_or(&default_opts);
    save_bytes_with(filename, doc, password, export_opts)
}

/// Exports document to PDF bytes using WordCraft's krilla-based PDF engine.
pub fn export_document_pdf(
    doc: &Document,
    tagged: bool,
    markup: bool,
    comments: bool,
) -> Result<Vec<u8>, String> {
    let pdf_opts = wordcraft_pdf::PdfOptions {
        tagged,
        include_markup: markup,
        comments,
        ..Default::default()
    };
    wordcraft_pdf::export(doc, &pdf_opts).map_err(|e| format!("Failed to export PDF: {e}"))
}


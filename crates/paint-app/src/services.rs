use std::sync::Arc;

use paint_domain::ports::{FontSource, ImageExporter, ImageImporter, ProjectStore};

/// Infrastructure adapters injected by the composition root (`paint-ui`).
#[derive(Debug)]
pub struct Services {
    pub project_store: Box<dyn ProjectStore>,
    pub image_importer: Box<dyn ImageImporter>,
    pub image_exporter: Box<dyn ImageExporter>,
    /// Installed fonts for the text tool.
    pub fonts: Arc<dyn FontSource>,
}

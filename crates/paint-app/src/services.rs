use paint_domain::ports::{ImageExporter, ImageImporter, ProjectStore};

/// Infrastructure adapters injected by the composition root (`paint-ui`).
#[derive(Debug)]
pub struct Services {
    pub project_store: Box<dyn ProjectStore>,
    pub image_importer: Box<dyn ImageImporter>,
    pub image_exporter: Box<dyn ImageExporter>,
}

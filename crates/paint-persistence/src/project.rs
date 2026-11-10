//! `.paint` project format, version 1 (UTF-8 JSON, see `docs/FileFormat.md`).
//!
//! The serialized structure is a set of private DTOs separate from the
//! domain types, so the on-disk format can evolve (new `version`s) without
//! leaking serde into `paint-domain`.

use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use paint_domain::ports::{PersistenceError, ProjectStore};
use paint_domain::{
    CanvasSize, Color, Document, DrawOp, Layer, LayerContent, LayerId, PixelRect, Point,
    StrokeWidth, TextSize,
};
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomically;
use crate::image_io::{decode_image, encode_png};

/// Value of the `format` field identifying a project file.
pub const FORMAT_ID: &str = "rust-paint-project";
/// Current (and newest readable) format version.
pub const FORMAT_VERSION: u32 = 1;
/// File extension of project files (without the dot).
pub const PROJECT_EXTENSION: &str = "paint";

#[derive(Debug, Serialize, Deserialize)]
struct ProjectDto {
    format: String,
    version: u32,
    canvas: CanvasDto,
    layers: Vec<LayerDto>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanvasDto {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct LayerDto {
    id: u32,
    name: String,
    visible: bool,
    locked: bool,
    content: ContentDto,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDto {
    Raster {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        png_base64: Option<String>,
    },
    Vector {
        ops: Vec<OpDto>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpDto {
    Stroke {
        color: [u8; 4],
        width: u32,
        points: Vec<[f32; 2]>,
    },
    Erase {
        width: u32,
        points: Vec<[f32; 2]>,
    },
    ClearRect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Text {
        text: String,
        origin: [f32; 2],
        size: u16,
        color: [u8; 4],
        font: String,
    },
}

/// Serializes a document to `.paint` bytes.
pub fn encode_project(document: &Document) -> Result<Vec<u8>, PersistenceError> {
    let canvas = document.canvas();
    let layers = document
        .layers()
        .iter()
        .map(layer_to_dto)
        .collect::<Result<_, _>>()?;
    let dto = ProjectDto {
        format: FORMAT_ID.to_owned(),
        version: FORMAT_VERSION,
        canvas: CanvasDto {
            width: canvas.width,
            height: canvas.height,
        },
        layers,
    };
    serde_json::to_vec_pretty(&dto).map_err(|e| PersistenceError::InvalidFormat(e.to_string()))
}

/// Parses and validates `.paint` bytes. Never panics on malformed input.
pub fn decode_project(bytes: &[u8]) -> Result<Document, PersistenceError> {
    let dto: ProjectDto = serde_json::from_slice(bytes)
        .map_err(|e| PersistenceError::InvalidFormat(e.to_string()))?;
    if dto.format != FORMAT_ID {
        return Err(PersistenceError::InvalidFormat(
            "お絵描きプロジェクトファイルではありません".to_owned(),
        ));
    }
    if dto.version != FORMAT_VERSION {
        return Err(PersistenceError::Unsupported(format!(
            "プロジェクト形式 version {} (対応: {FORMAT_VERSION})",
            dto.version
        )));
    }
    let canvas = CanvasSize::new(dto.canvas.width, dto.canvas.height)?;
    let layers = dto
        .layers
        .into_iter()
        .map(|l| layer_from_dto(l, canvas))
        .collect::<Result<_, _>>()?;
    Ok(Document::from_parts(canvas, layers)?)
}

fn layer_to_dto(layer: &Layer) -> Result<LayerDto, PersistenceError> {
    let content = match &layer.content {
        LayerContent::Raster(image) => ContentDto::Raster {
            png_base64: image
                .as_deref()
                .map(|img| encode_png(img).map(|png| BASE64.encode(png)))
                .transpose()?,
        },
        LayerContent::Vector(ops) => ContentDto::Vector {
            ops: ops.iter().map(op_to_dto).collect(),
        },
    };
    Ok(LayerDto {
        id: layer.id.0,
        name: layer.name.clone(),
        visible: layer.visible,
        locked: layer.locked,
        content,
    })
}

fn layer_from_dto(dto: LayerDto, canvas: CanvasSize) -> Result<Layer, PersistenceError> {
    let content = match dto.content {
        ContentDto::Raster { png_base64 } => {
            let image = png_base64
                .map(|b64| {
                    let png = BASE64
                        .decode(b64)
                        .map_err(|e| PersistenceError::InvalidFormat(e.to_string()))?;
                    decode_image(&png)
                })
                .transpose()?;
            LayerContent::Raster(image.map(Arc::new))
        }
        ContentDto::Vector { ops } => LayerContent::Vector(
            ops.into_iter()
                .map(|op| op_from_dto(op, canvas))
                .collect::<Result<_, _>>()?,
        ),
    };
    Ok(Layer {
        id: LayerId(dto.id),
        name: dto.name,
        visible: dto.visible,
        locked: dto.locked,
        content,
    })
}

fn op_to_dto(op: &DrawOp) -> OpDto {
    let points = |pts: &[Point]| pts.iter().map(|p| [p.x, p.y]).collect();
    match op {
        DrawOp::Stroke {
            color,
            width,
            points: pts,
        } => OpDto::Stroke {
            color: color.to_array(),
            width: width.px(),
            points: points(pts),
        },
        DrawOp::Erase { width, points: pts } => OpDto::Erase {
            width: width.px(),
            points: points(pts),
        },
        DrawOp::ClearRect { rect } => OpDto::ClearRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        },
        DrawOp::Text {
            text,
            origin,
            size,
            color,
            font,
        } => OpDto::Text {
            text: text.clone(),
            origin: [origin.x, origin.y],
            size: size.px(),
            color: color.to_array(),
            font: font.clone(),
        },
    }
}

fn op_from_dto(dto: OpDto, canvas: CanvasSize) -> Result<DrawOp, PersistenceError> {
    let width = |px: u32| {
        StrokeWidth::from_px(px)
            .ok_or_else(|| PersistenceError::InvalidFormat(format!("未対応の線幅 {px}px")))
    };
    let points = |pts: Vec<[f32; 2]>| -> Result<Vec<Point>, PersistenceError> {
        if pts.is_empty() {
            return Err(PersistenceError::InvalidFormat(
                "点のない描画操作".to_owned(),
            ));
        }
        pts.into_iter()
            .map(|[x, y]| {
                let p = Point::new(x, y);
                p.is_finite()
                    .then_some(p)
                    .ok_or_else(|| PersistenceError::InvalidFormat("座標が不正です".to_owned()))
            })
            .collect()
    };
    Ok(match dto {
        OpDto::Stroke {
            color,
            width: w,
            points: pts,
        } => DrawOp::Stroke {
            color: Color::from_array(color),
            width: width(w)?,
            points: points(pts)?,
        },
        OpDto::Erase {
            width: w,
            points: pts,
        } => DrawOp::Erase {
            width: width(w)?,
            points: points(pts)?,
        },
        OpDto::ClearRect {
            x,
            y,
            width,
            height,
        } => {
            let rect = PixelRect::new(x, y, width, height);
            if rect.is_empty() || rect.clamp_to(canvas) != rect {
                return Err(PersistenceError::InvalidFormat(
                    "消去範囲がキャンバス外です".to_owned(),
                ));
            }
            DrawOp::ClearRect { rect }
        }
        OpDto::Text {
            text,
            origin: [x, y],
            size,
            color,
            font,
        } => {
            let origin = Point::new(x, y);
            if text.is_empty() || !origin.is_finite() {
                return Err(PersistenceError::InvalidFormat(
                    "テキスト操作が不正です".to_owned(),
                ));
            }
            DrawOp::Text {
                text,
                origin,
                size: TextSize::new(size).ok_or_else(|| {
                    PersistenceError::InvalidFormat(format!("未対応の文字サイズ {size}px"))
                })?,
                color: Color::from_array(color),
                font,
            }
        }
    })
}

/// [`ProjectStore`] reading and writing `.paint` JSON files.
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonProjectStore;

impl ProjectStore for JsonProjectStore {
    fn save(&self, path: &Path, document: &Document) -> Result<(), PersistenceError> {
        write_atomically(path, &encode_project(document)?)?;
        Ok(())
    }

    fn load(&self, path: &Path) -> Result<Document, PersistenceError> {
        decode_project(&std::fs::read(path)?)
    }
}

#[cfg(test)]
mod tests {
    use paint_domain::{RasterImage, Underlay};

    use super::*;

    const CANVAS: CanvasSize = CanvasSize {
        width: 64,
        height: 48,
    };

    fn sample_document() -> Document {
        let mut doc = Document::new(CANVAS);
        let mut underlay = RasterImage::filled(CANVAS, Color::WHITE);
        underlay.set_pixel(3, 4, Color::rgba(10, 20, 30, 128));
        doc.replace_underlay(Underlay {
            image: Some(Arc::new(underlay)),
            canvas: CANVAS,
        })
        .unwrap();
        let layer = Document::DRAWING_LAYER;
        doc.push_op(
            layer,
            DrawOp::Stroke {
                color: Color::rgba(255, 0, 0, 200),
                width: StrokeWidth::Px4,
                points: vec![Point::new(1.5, 2.25), Point::new(30.0, 40.0)],
            },
        )
        .unwrap();
        doc.push_op(
            layer,
            DrawOp::Erase {
                width: StrokeWidth::Px16,
                points: vec![Point::new(12.0, 22.0)],
            },
        )
        .unwrap();
        doc.push_op(
            layer,
            DrawOp::ClearRect {
                rect: PixelRect::new(0, 0, 10, 5),
            },
        )
        .unwrap();
        doc.push_op(
            layer,
            DrawOp::Text {
                text: "日本語\nEnglish".to_owned(),
                origin: Point::new(4.5, 6.0),
                size: TextSize::new(123).unwrap(),
                color: Color::rgba(1, 2, 3, 200),
                font: "游ゴシック".to_owned(),
            },
        )
        .unwrap();
        doc.set_layer_visible(layer, false).unwrap();
        doc
    }

    fn json(doc: &Document) -> serde_json::Value {
        serde_json::from_slice(&encode_project(doc).unwrap()).unwrap()
    }

    fn decode_value(value: &serde_json::Value) -> Result<Document, PersistenceError> {
        decode_project(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn round_trip_preserves_everything() {
        let doc = sample_document();
        assert_eq!(decode_project(&encode_project(&doc).unwrap()).unwrap(), doc);
    }

    #[test]
    fn round_trip_without_underlay() {
        let doc = Document::new(CanvasSize::DEFAULT);
        let value = json(&doc);
        assert!(value["layers"][0]["content"].get("png_base64").is_none());
        assert_eq!(decode_value(&value).unwrap(), doc);
    }

    #[test]
    fn encodes_documented_layout() {
        let value = json(&sample_document());
        assert_eq!(value["format"], FORMAT_ID);
        assert_eq!(value["version"], 1);
        assert_eq!(value["canvas"]["width"], 64);
        assert_eq!(value["layers"][0]["locked"], true);
        assert_eq!(value["layers"][0]["content"]["type"], "raster");
        let layer = &value["layers"][1];
        assert_eq!(layer["visible"], false);
        let ops = &layer["content"]["ops"];
        assert_eq!(ops[0]["type"], "stroke");
        assert_eq!(ops[0]["color"], serde_json::json!([255, 0, 0, 200]));
        assert_eq!(ops[0]["width"], 4);
        assert_eq!(ops[0]["points"][0], serde_json::json!([1.5, 2.25]));
        assert_eq!(ops[1]["type"], "erase");
        assert_eq!(ops[1]["width"], 16);
        assert_eq!(ops[2]["type"], "clear_rect");
        assert_eq!(ops[2]["height"], 5);
    }

    #[test]
    fn rejects_wrong_format_and_version() {
        let mut value = json(&sample_document());
        value["format"] = "something-else".into();
        assert!(matches!(
            decode_value(&value),
            Err(PersistenceError::InvalidFormat(_))
        ));
        let mut value = json(&sample_document());
        value["version"] = 2.into();
        assert!(matches!(
            decode_value(&value),
            Err(PersistenceError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_invalid_contents() {
        let cases: [(&str, serde_json::Value); 10] = [
            ("/canvas/width", 0.into()),
            ("/canvas/width", 5000.into()),
            ("/layers/1/content/ops/0/width", 3.into()),
            ("/layers/1/content/ops/0/points", serde_json::json!([])),
            ("/layers/1/content/ops/2/width", 1000.into()),
            ("/layers/0/content/png_base64", "%%%".into()),
            ("/layers/1/id", 0.into()),
            ("/layers/1/content/ops/3/size", 7.into()),
            ("/layers/1/content/ops/3/size", 401.into()),
            ("/layers/1/content/ops/3/text", "".into()),
        ];
        for (pointer, replacement) in cases {
            let mut value = json(&sample_document());
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                decode_value(&value).is_err(),
                "{pointer} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_underlay_with_mismatched_size() {
        let mut value = json(&sample_document());
        value["canvas"]["width"] = 32.into();
        assert!(decode_value(&value).is_err());
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        for bytes in [
            &b""[..],
            b"{",
            b"null",
            b"[]",
            b"\xff\xfe",
            b"{\"format\":1}",
        ] {
            assert!(decode_project(bytes).is_err());
        }
    }

    #[test]
    fn store_saves_and_loads_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("work.paint");
        let doc = sample_document();
        JsonProjectStore.save(&path, &doc).unwrap();
        assert_eq!(JsonProjectStore.load(&path).unwrap(), doc);
        assert!(matches!(
            JsonProjectStore.load(&dir.path().join("missing.paint")),
            Err(PersistenceError::Io(_))
        ));
    }
}

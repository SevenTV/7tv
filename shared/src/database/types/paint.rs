use std::collections::HashMap;
use std::sync::Arc;

use bson::oid::ObjectId;

use super::{FileSet, FileSetProperties, ImageFormat};
use crate::database::Collection;
use crate::types::old::{
	CosmeticPaintFunction, CosmeticPaintGradientStop, CosmeticPaintModel, CosmeticPaintShadow, CosmeticPaintShape,
	ImageFormat as ImageFormatOld, ImageHost, ImageHostKind,
};

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Paint {
	#[serde(rename = "_id")]
	pub id: ObjectId,
	pub name: String,
	pub description: String,
	pub tags: Vec<String>,
	pub data: PaintData,
	pub file_set_ids: Vec<ObjectId>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Collection for Paint {
	const COLLECTION_NAME: &'static str = "paints";
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PaintData {
	pub layers: Vec<PaintLayer>,
	pub shadows: Vec<PaintShadow>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PaintLayer {
	#[serde(flatten)]
	pub ty: PaintLayerType,
	pub opacity: f64,
}

impl Default for PaintLayer {
	fn default() -> Self {
		Self {
			ty: PaintLayerType::default(),
			opacity: 1.0,
		}
	}
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum PaintLayerType {
	SingleColor(u32),
	LinearGradient {
		angle: i32,
		repeating: bool,
		stops: Vec<PaintGradientStop>,
	},
	RadialGradient {
		angle: i32,
		repeating: bool,
		stops: Vec<PaintGradientStop>,
		shape: PaintRadialGradientShape,
	},
	Image(ObjectId),
}

impl Default for PaintLayerType {
	fn default() -> Self {
		Self::SingleColor(0xffffff)
	}
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PaintGradientStop {
	pub at: f64,
	pub color: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum PaintRadialGradientShape {
	#[default]
	Ellipse,
	Circle,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PaintShadow {
	pub color: u32,
	pub offset_x: f64,
	pub offset_y: f64,
	pub blur: f64,
}

impl From<PaintShadow> for CosmeticPaintShadow {
	fn from(s: PaintShadow) -> Self {
		Self {
			color: s.color as i32,
			x_offset: s.offset_x,
			y_offset: s.offset_y,
			radius: s.blur,
		}
	}
}

impl Paint {
	pub fn into_old_model(self, files: &HashMap<ObjectId, FileSet>, cdn_base_url: &str) -> Option<CosmeticPaintModel> {
		let first_layer = self.data.layers.first();

		Some(CosmeticPaintModel {
			id: self.id,
			name: self.name,
			color: first_layer.and_then(|l| match l.ty {
				PaintLayerType::SingleColor(c) => Some(c as i32),
				_ => None,
			}),
			gradients: vec![],
			shadows: self.data.shadows.into_iter().map(|s| s.into()).collect(),
			text: None,
			function: first_layer
				.map(|l| match l.ty {
					PaintLayerType::SingleColor(..) => CosmeticPaintFunction::LinearGradient,
					PaintLayerType::LinearGradient { .. } => CosmeticPaintFunction::LinearGradient,
					PaintLayerType::RadialGradient { .. } => CosmeticPaintFunction::RadialGradient,
					PaintLayerType::Image(..) => CosmeticPaintFunction::Url,
				})
				.unwrap_or(CosmeticPaintFunction::LinearGradient),
			repeat: first_layer
				.map(|l| match l.ty {
					PaintLayerType::LinearGradient { repeating, .. } | PaintLayerType::RadialGradient { repeating, .. } => {
						repeating
					}
					_ => false,
				})
				.unwrap_or_default(),
			angle: first_layer
				.and_then(|l| match l.ty {
					PaintLayerType::LinearGradient { angle, .. } | PaintLayerType::RadialGradient { angle, .. } => {
						Some(angle)
					}
					_ => None,
				})
				.unwrap_or_default(),
			shape: first_layer
				.and_then(|l| match l.ty {
					PaintLayerType::RadialGradient {
						shape: PaintRadialGradientShape::Ellipse,
						..
					} => Some(CosmeticPaintShape::Ellipse),
					PaintLayerType::RadialGradient {
						shape: PaintRadialGradientShape::Circle,
						..
					} => Some(CosmeticPaintShape::Circle),
					_ => None,
				})
				.unwrap_or_default(),
			image_url: first_layer
				.and_then(|l| match l.ty {
					PaintLayerType::Image(id) => files.get(&id).and_then(|f| {
						f.properties.default_image().and_then(|file| {
							Some(
								ImageHostKind::Paint.create_full_url(
									cdn_base_url,
									id,
									file.extra.scale,
									file.extra
										.variants
										.iter()
										.find(|v| v.format == ImageFormat::Webp)
										.map(|_| ImageFormatOld::Webp)?,
								),
							)
						})
					}),
					_ => None,
				})
				.unwrap_or_default(),
			stops: first_layer
				.and_then(|l| match &l.ty {
					PaintLayerType::LinearGradient { stops, .. } | PaintLayerType::RadialGradient { stops, .. } => Some(
						stops
							.iter()
							.map(|s| CosmeticPaintGradientStop {
								color: s.color as i32,
								at: s.at,
								center_at: [0.0, 0.0],
							})
							.collect(),
					),
					_ => None,
				})
				.unwrap_or_default(),
		})
	}
}

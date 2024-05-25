use anyhow::Context;
use axum::body::Bytes;
use image_processor::{OutputFormat, OutputFormatOptions, OutputQuality};
use scuffle_image_processor_proto::image_processor_client::ImageProcessorClient;
use scuffle_image_processor_proto::{self as image_processor};
use shared::database::{EmoteId, Id, UserId};

use crate::config::ImageProcessorConfig;

pub mod callback;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
	Emote(EmoteId),
	ProfilePicture(UserId),
	Wildcard,
}

impl Subject {
	pub fn to_string(&self, prefix: &str) -> String {
		let mut parts: Vec<String> = Vec::new();

		if !prefix.is_empty() {
			parts.push(prefix.to_string());
		}

		match self {
			Self::Emote(id) => {
				parts.push("emote".to_string());
				parts.push(id.to_string());
			},
			Self::ProfilePicture(id) => {
				parts.push("profile-picture".to_string());
				parts.push(id.to_string());
			},
			Self::Wildcard => {
				parts.push(">".to_string());
			}
		}

		parts.join(".")
	}

	pub fn from_string(s: &str, prefix: &str) -> anyhow::Result<Self> {
		let mut parts = s.split('.');

		if !prefix.is_empty() {
			if parts.next().context("no prefix")? != prefix {
				anyhow::bail!("invalid prefix");
			}
		}

		match (parts.next().context("subject too short")?, parts.next()) {
			("emote", Some(id)) => Ok(Self::Emote(id.parse()?)),
			("profile-picture", Some(id)) => Ok(Self::ProfilePicture(id.parse()?)),
			(">", None) => Ok(Self::Wildcard),
			_ => anyhow::bail!("invalid subject"),
		}
	}
}

pub struct ImageProcessor {
	client: ImageProcessorClient<tonic::transport::Channel>,
	input_drive_name: String,
	output_drive_name: String,
	event_queue_name: String,
	event_queue_topic_prefix: String,
}

impl ImageProcessor {
	pub async fn new(config: &ImageProcessorConfig) -> anyhow::Result<Self> {
		let channel =
			shared::grpc::make_channel(config.address.clone(), config.resolve_interval, None).context("make channel")?;

		let client = ImageProcessorClient::new(channel);

		Ok(Self {
			client,
			input_drive_name: config.input_drive_name.clone(),
			output_drive_name: config.output_drive_name.clone(),
			event_queue_name: config.event_queue_name.clone(),
			event_queue_topic_prefix: config.event_queue_topic_prefix.clone(),
		})
	}

	pub async fn upload_emote(
		&self,
		id: EmoteId,
		data: Bytes,
	) -> tonic::Result<scuffle_image_processor_proto::ProcessImageResponse> {
		let topic = Subject::Emote(id).to_string(&self.event_queue_topic_prefix);

		let request = image_processor::ProcessImageRequest {
			input_upload: Some(image_processor::InputUpload {
				drive_path: Some(image_processor::DrivePath {
					drive: self.input_drive_name.clone(),
					path: format!("/emote/{id}/input.{{ext}}"),
				}),
				acl: Some("private".to_string()),
				binary: data.to_vec(),
				..Default::default()
			}),
			task: Some(image_processor::Task {
				output: Some(image_processor::Output {
					drive_path: Some(image_processor::DrivePath {
						drive: self.output_drive_name.clone(),
						path: format!("/emote/{id}/{{scale}}x{{static}}.{{ext}}"),
					}),
					formats: vec![
						OutputFormatOptions {
							format: OutputFormat::WebpAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::WebpStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::AvifAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::AvifStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::GifAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::PngStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
					],
					upscale: true,
					skip_impossible_formats: true,
					min_aspect_ratio: None,
					max_aspect_ratio: Some(3.0),
					resize_method: image_processor::ResizeMethod::Fit as i32,
					resize_algorithm: image_processor::ResizeAlgorithm::Lanczos3 as i32,
					resize: Some(image_processor::output::Resize::Scaling(image_processor::Scaling {
						base: Some(image_processor::scaling::Base::BaseHeight(32)),
						scales: vec![1, 2, 3, 4],
					})),
					..Default::default()
				}),
				events: Some(image_processor::Events {
					on_success: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_start: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_failure: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_cancel: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					metadata: [("emote_id".to_string(), id.to_string())].into_iter().collect(),
					..Default::default()
				}),
				limits: Some(image_processor::Limits {
					max_input_frame_count: Some(1000),
					max_input_width: Some(1000),
					max_input_height: Some(1000),
					..Default::default()
				}),
				..Default::default()
			}),
			priority: 5,
			..Default::default()
		};

		Ok(self.client.clone().process_image(request).await?.into_inner())
	}

	pub async fn upload_profile_picture(
		&self,
		id: UserId,
		data: Bytes,
	) -> tonic::Result<scuffle_image_processor_proto::ProcessImageResponse> {
		let topic = Subject::ProfilePicture(id).to_string(&self.event_queue_topic_prefix);

		// random id for the profile picture
		let pp_id = Id::<()>::new();

		let request = image_processor::ProcessImageRequest {
			input_upload: Some(image_processor::InputUpload {
				drive_path: Some(image_processor::DrivePath {
					drive: self.input_drive_name.clone(),
					path: format!("/user/{id}/profile-picture/{pp_id}/input.{{ext}}"),
				}),
				acl: Some("private".to_string()),
				binary: data.to_vec(),
				..Default::default()
			}),
			task: Some(image_processor::Task {
				output: Some(image_processor::Output {
					drive_path: Some(image_processor::DrivePath {
						drive: self.output_drive_name.clone(),
						path: format!("/user/{id}/profile-picture/{pp_id}/{{scale}}x{{static}}.{{ext}}"),
					}),
					formats: vec![
						OutputFormatOptions {
							format: OutputFormat::WebpAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::WebpStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::AvifAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::AvifStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::GifAnim as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
						OutputFormatOptions {
							format: OutputFormat::PngStatic as i32,
							quality: OutputQuality::Auto as i32,
							name: None,
						},
					],
					upscale: true,
					skip_impossible_formats: true,
					min_aspect_ratio: None,
					max_aspect_ratio: Some(3.0),
					resize_method: image_processor::ResizeMethod::Fit as i32,
					resize_algorithm: image_processor::ResizeAlgorithm::Lanczos3 as i32,
					resize: Some(image_processor::output::Resize::Scaling(image_processor::Scaling {
						base: Some(image_processor::scaling::Base::BaseHeight(32)),
						scales: vec![1, 2, 3, 4],
					})),
					..Default::default()
				}),
				events: Some(image_processor::Events {
					on_success: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_start: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_failure: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					on_cancel: Some(image_processor::EventQueue {
						name: self.event_queue_name.clone(),
						topic: topic.clone(),
					}),
					metadata: [("user_id".to_string(), id.to_string())].into_iter().collect(),
					..Default::default()
				}),
				limits: Some(image_processor::Limits {
					max_input_frame_count: Some(1000),
					max_input_width: Some(1000),
					max_input_height: Some(1000),
					..Default::default()
				}),
				..Default::default()
			}),
			priority: 5,
			..Default::default()
		};

		Ok(self.client.clone().process_image(request).await?.into_inner())
	}
}

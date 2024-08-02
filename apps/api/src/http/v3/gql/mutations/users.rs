use std::sync::Arc;

use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};
use itertools::Itertools;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use shared::database::entitlement::{EntitlementEdge, EntitlementEdgeId, EntitlementEdgeKind};
use shared::database::event::{EventEntitlementEdgeData, EventUserEditorData};
use shared::database::queries::{filter, update};
use shared::database::role::permissions::{AdminPermission, PermissionsExt, RolePermission, UserPermission};
use shared::database::user::connection::UserConnection as DbUserConnection;
use shared::database::user::editor::{UserEditor as DbUserEditor, UserEditorId, UserEditorState};
use shared::database::user::{User, UserStyle};
use shared::database::MongoCollection;
use shared::event::{EventPayload, EventPayloadData};
use shared::event_api::types::{ChangeField, ChangeFieldType, ChangeMap, EventType, ObjectKind};
use shared::old_types::cosmetic::CosmeticKind;
use shared::old_types::object_id::GqlObjectId;
use shared::old_types::UserPartialModel;

use crate::global::Global;
use crate::http::error::ApiError;
use crate::http::middleware::auth::AuthSession;
use crate::http::v3::emote_set_loader::load_emote_set;
use crate::http::v3::gql::guards::PermissionGuard;
use crate::http::v3::gql::queries::user::{UserConnection, UserEditor};
use crate::http::v3::gql::types::ListItemAction;
use crate::http::v3::rest::types::EmoteSetModel;
use crate::http::v3::types::UserEditorModelPermission;
use crate::transactions::{with_transaction, TransactionError};

#[derive(Default)]
pub struct UsersMutation;

#[Object(rename_fields = "camelCase", rename_args = "snake_case")]
impl UsersMutation {
	async fn user(&self, id: GqlObjectId) -> UserOps {
		UserOps { id }
	}
}

#[derive(SimpleObject)]
#[graphql(complex, rename_fields = "snake_case")]
pub struct UserOps {
	id: GqlObjectId,
}

#[ComplexObject(rename_fields = "camelCase", rename_args = "snake_case")]
impl UserOps {
	async fn connections<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		id: String,
		data: UserConnectionUpdate,
	) -> Result<Option<Vec<Option<UserConnection>>>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let authed_user = auth_session.user(global).await?;

		if !(auth_session.user_id() == self.id.id() || authed_user.has(UserPermission::ManageAny)) {
			return Err(ApiError::FORBIDDEN);
		}

		let old_user = global
			.user_loader
			.load(global, self.id.id())
			.await
			.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or(ApiError::NOT_FOUND)?;

		let emote_set = if let Some(emote_set_id) = data.emote_set_id {
			// check if set exists
			let emote_set = global
				.emote_set_by_id_loader
				.load(emote_set_id.id())
				.await
				.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?
				.ok_or(ApiError::NOT_FOUND)?;

			Some(emote_set)
		} else {
			None
		};

		let update_pull = if let Some(true) = data.unlink {
			Some(update::update! {
				#[query(pull)]
				User {
					connections: DbUserConnection {
						platform_id: id.clone(),
					}
				}
			})
		} else {
			None
		};

		let Some(user) = User::collection(&global.db)
			.find_one_and_update(
				filter::filter! {
					User {
						#[query(rename = "_id")]
						id: self.id.id(),
					}
				},
				update::update! {
					#[query(set)]
					User {
						#[query(flatten)]
						style: UserStyle {
							#[query(optional)]
							active_emote_set_id: data.emote_set_id.map(|id| id.id()),
						},
						updated_at: chrono::Utc::now(),
					},
					#[query(pull)]
					update_pull,
				},
			)
			.return_document(ReturnDocument::After)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to update user");
				ApiError::INTERNAL_SERVER_ERROR
			})?
		else {
			return Ok(None);
		};

		if let Some(true) = data.unlink {
			let (index, connection) = old_user
				.connections
				.iter()
				.find_position(|c| c.platform_id == id)
				.ok_or(ApiError::NOT_FOUND)?;

			global
				.event_api
				.dispatch_event(
					EventType::UpdateUser,
					ChangeMap {
						id: self.id.0.cast(),
						kind: ObjectKind::User,
						actor: Some(UserPartialModel::from_db(
							authed_user.clone(),
							None,
							None,
							&global.config.api.cdn_origin,
						)),
						pulled: vec![ChangeField {
							key: "connections".to_string(),
							ty: ChangeFieldType::Object,
							index: Some(index),
							value: serde_json::to_value(connection).map_err(|e| {
								tracing::error!(error = %e, "failed to serialize value");
								ApiError::INTERNAL_SERVER_ERROR
							})?,
							..Default::default()
						}],
						..Default::default()
					},
					self.id.0,
				)
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to dispatch event");
					ApiError::INTERNAL_SERVER_ERROR
				})?;
		}

		if let Some(emote_set) = emote_set {
			let old_set = match old_user.style.active_emote_set_id {
				Some(id) => {
					let set = global.emote_set_by_id_loader.load(id).await.map_err(|_| {
						tracing::error!("failed to load old emote set");
						ApiError::INTERNAL_SERVER_ERROR
					})?;

					if let Some(set) = set {
						let emotes = load_emote_set(global, set.emotes.clone(), None, false)
							.await
							.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

						Some(EmoteSetModel::from_db(set, emotes, None))
					} else {
						None
					}
				}
				None => None,
			};
			let old_set = serde_json::to_value(old_set).map_err(|e| {
				tracing::error!(error = %e, "failed to serialize value");
				ApiError::INTERNAL_SERVER_ERROR
			})?;

			let new_set_id = emote_set.id;
			let emotes = load_emote_set(global, emote_set.emotes.clone(), None, false)
				.await
				.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;
			let new_set = EmoteSetModel::from_db(emote_set, emotes, None);
			let new_set = serde_json::to_value(new_set).map_err(|e| {
				tracing::error!(error = %e, "failed to serialize value");
				ApiError::INTERNAL_SERVER_ERROR
			})?;

			for i in 0..user.connections.len() {
				let value = vec![
					ChangeField {
						key: "emote_set".to_string(),
						ty: ChangeFieldType::Object,
						old_value: old_set.clone(),
						value: new_set.clone(),
						..Default::default()
					},
					ChangeField {
						key: "emote_set_id".to_string(),
						ty: ChangeFieldType::String,
						old_value: user.style.active_emote_set_id.map(|id| id.to_string()).into(),
						value: new_set_id.to_string().into(),
						..Default::default()
					},
				];

				let value = serde_json::to_value(value).map_err(|e| {
					tracing::error!(error = %e, "failed to serialize value");
					ApiError::INTERNAL_SERVER_ERROR
				})?;

				global
					.event_api
					.dispatch_event(
						EventType::UpdateUser,
						ChangeMap {
							id: self.id.0.cast(),
							kind: ObjectKind::User,
							actor: Some(UserPartialModel::from_db(
								authed_user.clone(),
								None,
								None,
								&global.config.api.cdn_origin,
							)),
							updated: vec![ChangeField {
								key: "connections".to_string(),
								index: Some(i),
								nested: true,
								value,
								..Default::default()
							}],
							..Default::default()
						},
						self.id.0,
					)
					.await
					.map_err(|e| {
						tracing::error!(error = %e, "failed to dispatch event");
						ApiError::INTERNAL_SERVER_ERROR
					})?;
			}
		}

		let full_user = global
			.user_loader
			.load_fast_user(global, user)
			.await
			.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?;

		Ok(Some(
			full_user
				.connections
				.iter()
				.cloned()
				.map(|c| {
					Some(UserConnection::from_db(
						full_user.computed.permissions.emote_set_capacity.unwrap_or_default(),
						c,
						&full_user.style,
					))
				})
				.collect(),
		))
	}

	async fn editors(
		&self,
		ctx: &Context<'_>,
		editor_id: GqlObjectId,
		data: UserEditorUpdate,
	) -> Result<Option<Vec<Option<UserEditor>>>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let user = auth_session.user(global).await?;

		if !(auth_session.user_id() == self.id.id() || user.has(UserPermission::ManageAny)) {
			return Err(ApiError::FORBIDDEN);
		}

		let res = with_transaction(global, |mut tx| async move {
			if let Some(permissions) = data.permissions {
				if permissions == UserEditorModelPermission::none() {
					let editor_id = UserEditorId {
						user_id: self.id.id(),
						editor_id: editor_id.id(),
					};

					// Remove editor
					let res = tx
						.find_one_and_delete(
							filter::filter! {
								DbUserEditor {
									#[query(serde, rename = "_id")]
									id: editor_id,
								}
							},
							None,
						)
						.await?;

					if let Some(editor) = res {
						tx.register_event(EventPayload {
							actor_id: Some(auth_session.user_id()),
							data: EventPayloadData::UserEditor {
								after: editor,
								data: EventUserEditorData::RemoveEditor {
									editor_id: editor_id.editor_id,
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					}
				} else {
					// Add or update editor
					let editor_id = UserEditorId {
						user_id: self.id.id(),
						editor_id: editor_id.id(),
					};

					let permissions = permissions.to_db();

					let editor = tx
						.find_one_and_update(
							filter::filter! {
								DbUserEditor {
									#[query(serde, rename = "_id")]
									id: editor_id,
								}
							},
							update::update! {
								#[query(set)]
								DbUserEditor {
									#[query(serde)]
									permissions: permissions.clone(),
									updated_at: chrono::Utc::now(),
								}
							},
							FindOneAndUpdateOptions::builder()
								.return_document(ReturnDocument::After)
								.build(),
						)
						.await?;

					if let Some(editor) = editor {
						// updated
						tx.register_event(EventPayload {
							actor_id: Some(auth_session.user_id()),
							data: EventPayloadData::UserEditor {
								after: editor,
								data: EventUserEditorData::EditPermissions {
									old: todo!("query old permissions"),
									new: permissions,
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					} else {
						// didn't exist
						if user.has(UserPermission::InviteEditors) {
							return Err(TransactionError::custom(ApiError::FORBIDDEN));
						}

						let editor = DbUserEditor {
							id: editor_id,
							state: UserEditorState::Pending,
							notes: None,
							permissions: permissions.clone(),
							added_by_id: auth_session.user_id(),
							added_at: chrono::Utc::now(),
							updated_at: chrono::Utc::now(),
							search_updated_at: None,
						};

						tx.insert_one::<DbUserEditor>(&editor, None).await?;

						tx.register_event(EventPayload {
							actor_id: Some(auth_session.user_id()),
							data: EventPayloadData::UserEditor {
								after: editor,
								data: EventUserEditorData::AddEditor {
									editor_id: editor_id.editor_id,
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					}
				}
			}

			Ok(())
		})
		.await;

		match res {
			Ok(_) => {
				let editors = global
					.user_editor_by_user_id_loader
					.load(self.id.id())
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.unwrap_or_default();

				Ok(Some(
					editors
						.into_iter()
						.filter_map(|e| UserEditor::from_db(e, false))
						.map(Some)
						.collect(),
				))
			}
			Err(TransactionError::Custom(e)) => Err(e),
			Err(e) => {
				tracing::error!(error = %e, "transaction failed");
				Err(ApiError::INTERNAL_SERVER_ERROR)
			}
		}
	}

	async fn cosmetics<'ctx>(&self, ctx: &Context<'ctx>, update: UserCosmeticUpdate) -> Result<bool, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;

		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let authed_user = auth_session.user(global).await?;

		if !(auth_session.user_id() == self.id.id() || authed_user.has(UserPermission::ManageAny)) {
			return Err(ApiError::FORBIDDEN);
		}

		if !update.selected {
			return Ok(true);
		}

		let user = global
			.user_loader
			.load(global, self.id.id())
			.await
			.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or(ApiError::NOT_FOUND)?;

		let mut changes = vec![];

		let result = match update.kind {
			CosmeticKind::Paint => {
				// check if user has paint
				if !user.computed.entitlements.paints.contains(&update.id.id()) {
					return Err(ApiError::FORBIDDEN);
				}

				let old_paint = match user.style.active_paint_id {
					Some(paint_id) => global
						.paint_by_id_loader
						.load(paint_id)
						.await
						.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?,
					None => None,
				};

				let paint = global
					.paint_by_id_loader
					.load(update.id.id())
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::NOT_FOUND)?;

				let res = User::collection(&global.db)
					.update_one(
						filter::filter! {
							User {
								#[query(rename = "_id")]
								id: self.id.id(),
							}
						},
						update::update! {
							#[query(set)]
							User {
								#[query(flatten)]
								style: UserStyle {
									active_paint_id: update.id.id(),
								}
							}
						},
					)
					.await
					.map_err(|err| {
						tracing::error!(error = %err, "failed to update user");
						ApiError::INTERNAL_SERVER_ERROR
					})?;

				changes.push(ChangeField {
					key: "paint_id".to_string(),
					ty: ChangeFieldType::String,
					value: update.id.0.to_string().into(),
					old_value: user.style.active_paint_id.map(|id| id.to_string()).into(),
					..Default::default()
				});
				changes.push(ChangeField {
					key: "paint".to_string(),
					ty: ChangeFieldType::Object,
					value: serde_json::to_value(paint).map_err(|e| {
						tracing::error!(error = %e, "failed to serialize value");
						ApiError::INTERNAL_SERVER_ERROR
					})?,
					old_value: serde_json::to_value(old_paint).map_err(|e| {
						tracing::error!(error = %e, "failed to serialize value");
						ApiError::INTERNAL_SERVER_ERROR
					})?,
					..Default::default()
				});

				Ok(res.modified_count == 1)
			}
			CosmeticKind::Badge => {
				// check if user has paint
				if !user.computed.entitlements.badges.contains(&update.id.id()) {
					return Err(ApiError::FORBIDDEN);
				}

				let old_badge = match user.style.active_badge_id {
					Some(badge_id) => global
						.badge_by_id_loader
						.load(badge_id)
						.await
						.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?,
					None => None,
				};

				let badge = global
					.badge_by_id_loader
					.load(update.id.id())
					.await
					.map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?
					.ok_or(ApiError::NOT_FOUND)?;

				let res = User::collection(&global.db)
					.update_one(
						filter::filter! {
							User {
								#[query(rename = "_id")]
								id: self.id.id(),
							}
						},
						update::update! {
							#[query(set)]
							User {
								#[query(flatten)]
								style: UserStyle {
									active_badge_id: update.id.id(),
								}
							},
						},
					)
					.await
					.map_err(|err| {
						tracing::error!(error = %err, "failed to update user");
						ApiError::INTERNAL_SERVER_ERROR
					})?;

				changes.push(ChangeField {
					key: "badge_id".to_string(),
					ty: ChangeFieldType::String,
					value: update.id.0.to_string().into(),
					old_value: user.style.active_badge_id.map(|id| id.to_string()).into(),
					..Default::default()
				});
				changes.push(ChangeField {
					key: "badge".to_string(),
					ty: ChangeFieldType::Object,
					value: serde_json::to_value(badge).map_err(|e| {
						tracing::error!(error = %e, "failed to serialize value");
						ApiError::INTERNAL_SERVER_ERROR
					})?,
					old_value: serde_json::to_value(old_badge).map_err(|e| {
						tracing::error!(error = %e, "failed to serialize value");
						ApiError::INTERNAL_SERVER_ERROR
					})?,
					..Default::default()
				});

				Ok(res.modified_count == 1)
			}
			CosmeticKind::Avatar => Err(ApiError::NOT_IMPLEMENTED),
		};

		if let Ok(true) = result {
			global
				.event_api
				.dispatch_event(
					EventType::UpdateUser,
					ChangeMap {
						id: self.id.0.cast(),
						kind: ObjectKind::User,
						actor: Some(UserPartialModel::from_db(
							authed_user.clone(),
							None,
							None,
							&global.config.api.cdn_origin,
						)),
						updated: vec![ChangeField {
							key: "style".to_string(),
							ty: ChangeFieldType::Object,
							nested: true,
							value: serde_json::to_value(changes).map_err(|e| {
								tracing::error!(error = %e, "failed to serialize value");
								ApiError::INTERNAL_SERVER_ERROR
							})?,
							..Default::default()
						}],
						..Default::default()
					},
					self.id.0,
				)
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to dispatch event");
					ApiError::INTERNAL_SERVER_ERROR
				})?;
		}

		result
	}

	#[graphql(guard = "PermissionGuard::one(RolePermission::Assign)")]
	async fn roles<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		role_id: GqlObjectId,
		action: ListItemAction,
	) -> Result<Vec<GqlObjectId>, ApiError> {
		let global: &Arc<Global> = ctx.data().map_err(|_| ApiError::INTERNAL_SERVER_ERROR)?;
		let auth_session = ctx.data::<AuthSession>().map_err(|_| ApiError::UNAUTHORIZED)?;

		let user = auth_session.user(global).await?;

		let role = global
			.role_by_id_loader
			.load(role_id.id())
			.await
			.map_err(|()| ApiError::INTERNAL_SERVER_ERROR)?
			.ok_or(ApiError::NOT_FOUND)?;

		if role.permissions > user.computed.permissions && !user.has(AdminPermission::SuperAdmin) {
			return Err(ApiError::FORBIDDEN);
		}

		let mut changes = ChangeMap {
			id: self.id.0.cast(),
			kind: ObjectKind::User,
			actor: Some(UserPartialModel::from_db(
				auth_session.user(global).await?.clone(),
				None,
				None,
				&global.config.api.cdn_origin,
			)),
			..Default::default()
		};

		let res = with_transaction(global, |mut tx| async move {
			let roles = match action {
				ListItemAction::Add => {
					if user.computed.entitlements.roles.contains(&role_id.id()) {
						return Ok(user.computed.entitlements.roles.iter().copied().map(Into::into).collect());
					}

					let edge = EntitlementEdge {
						id: EntitlementEdgeId {
							from: EntitlementEdgeKind::User { user_id: self.id.id() },
							to: EntitlementEdgeKind::Role { role_id: role_id.id() },
							managed_by: None,
						},
					};

					tx.insert_one::<EntitlementEdge>(&edge, None).await?;

					tx.register_event(EventPayload {
						actor_id: Some(auth_session.user_id()),
						data: EventPayloadData::EntitlementEdge {
							after: edge,
							data: EventEntitlementEdgeData::Create,
						},
						timestamp: chrono::Utc::now(),
					})?;

					changes.pushed.push(ChangeField {
						key: "role_ids".to_string(),
						ty: ChangeFieldType::String,
						index: Some(role.rank as usize),
						value: role_id.0.to_string().into(),
						..Default::default()
					});

					user.computed
						.entitlements
						.roles
						.iter()
						.copied()
						.chain(std::iter::once(role_id.id()))
						.map(Into::into)
						.collect()
				}
				ListItemAction::Remove => {
					if let Some(edge) = tx
						.find_one_and_delete(
							filter::filter! {
								EntitlementEdge {
									#[query(serde)]
									id: EntitlementEdgeId {
										from: EntitlementEdgeKind::User { user_id: self.id.id() }.into(),
										to: EntitlementEdgeKind::Role { role_id: role_id.id() }.into(),
										managed_by: None,
									}
								}
							},
							None,
						)
						.await?
					{
						tx.register_event(EventPayload {
							actor_id: Some(auth_session.user_id()),
							data: EventPayloadData::EntitlementEdge {
								after: edge,
								data: EventEntitlementEdgeData::Delete,
							},
							timestamp: chrono::Utc::now(),
						})?;

						changes.pulled.push(ChangeField {
							key: "role_ids".to_string(),
							ty: ChangeFieldType::String,
							index: Some(role.rank as usize),
							value: role_id.0.to_string().into(),
							..Default::default()
						});

						user.computed
							.entitlements
							.roles
							.iter()
							.copied()
							.filter(|id| *id != role_id.id())
							.map(Into::into)
							.collect()
					} else {
						user.computed.entitlements.roles.iter().copied().map(Into::into).collect()
					}
				}
				ListItemAction::Update => return Err(TransactionError::custom(ApiError::NOT_IMPLEMENTED)),
			};

			global
				.event_api
				.dispatch_event(EventType::UpdateUser, changes, self.id.0)
				.await
				.map_err(|e| {
					tracing::error!(error = %e, "failed to dispatch event");
					ApiError::INTERNAL_SERVER_ERROR
				})
				.map_err(TransactionError::custom)?;

			Ok(roles)
		})
		.await;

		match res {
			Ok(roles) => Ok(roles),
			Err(TransactionError::Custom(e)) => Err(e),
			Err(e) => {
				tracing::error!(error = %e, "transaction failed");
				Err(ApiError::INTERNAL_SERVER_ERROR)
			}
		}
	}
}

#[derive(InputObject)]
#[graphql(rename_fields = "snake_case")]
pub struct UserConnectionUpdate {
	emote_set_id: Option<GqlObjectId>,
	unlink: Option<bool>,
}

#[derive(InputObject)]
#[graphql(rename_fields = "snake_case")]
pub struct UserEditorUpdate {
	permissions: Option<UserEditorModelPermission>,
	visible: Option<bool>,
}

#[derive(InputObject)]
#[graphql(rename_fields = "snake_case")]
pub struct UserCosmeticUpdate {
	id: GqlObjectId,
	kind: CosmeticKind,
	selected: bool,
}

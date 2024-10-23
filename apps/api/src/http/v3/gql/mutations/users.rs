use std::sync::Arc;

use async_graphql::{ComplexObject, Context, InputObject, Object, SimpleObject};
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument, UpdateOptions};
use shared::database::badge::BadgeId;
use shared::database::entitlement::{EntitlementEdge, EntitlementEdgeId, EntitlementEdgeKind};
use shared::database::paint::PaintId;
use shared::database::queries::{filter, update};
use shared::database::role::permissions::{PermissionsExt, RateLimitResource, RolePermission, UserPermission};
use shared::database::user::connection::UserConnection as DbUserConnection;
use shared::database::user::editor::{EditorUserPermission, UserEditor as DbUserEditor, UserEditorId, UserEditorState};
use shared::database::user::{User, UserStyle};
use shared::database::MongoCollection;
use shared::event::{InternalEvent, InternalEventData, InternalEventUserData, InternalEventUserEditorData};
use shared::old_types::cosmetic::CosmeticKind;
use shared::old_types::object_id::GqlObjectId;
use shared::old_types::UserEditorModelPermission;

use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};
use crate::http::middleware::session::Session;
use crate::http::v3::gql::guards::{PermissionGuard, RateLimitGuard};
use crate::http::v3::gql::queries::user::{UserConnection, UserEditor};
use crate::http::v3::gql::types::ListItemAction;
use crate::transactions::{transaction_with_mutex, GeneralMutexKey, TransactionError};

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
	#[graphql(guard = "RateLimitGuard::new(RateLimitResource::UserChangeConnections, 1)")]
	async fn connections<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		id: String,
		data: UserConnectionUpdate,
	) -> Result<Option<Vec<Option<UserConnection>>>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let session = ctx
			.data::<Session>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing sesion data"))?;
		let authed_user = session.user()?;

		if authed_user.id != self.id.id() && !authed_user.has(UserPermission::ManageAny) {
			let editor = global
				.user_editor_by_id_loader
				.load(UserEditorId {
					editor_id: authed_user.id,
					user_id: self.id.id(),
				})
				.await
				.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editor"))?
				.ok_or_else(|| {
					ApiError::forbidden(
						ApiErrorCode::LackingPrivileges,
						"you do not have permission to modify connections",
					)
				})?;

			if editor.state != UserEditorState::Accepted || !editor.permissions.has(EditorUserPermission::ManageProfile) {
				return Err(ApiError::forbidden(
					ApiErrorCode::LackingPrivileges,
					"you do not have permission to modify connections, you need the ManageProfile permission",
				));
			}
		}

		let res = transaction_with_mutex(
			global,
			Some(GeneralMutexKey::User(self.id.id()).into()),
			|mut tx| async move {
				let old_user = global
					.user_loader
					.load(global, self.id.id())
					.await
					.map_err(|_| {
						TransactionError::Custom(ApiError::internal_server_error(
							ApiErrorCode::LoadError,
							"failed to load user",
						))
					})?
					.ok_or_else(|| {
						TransactionError::Custom(ApiError::not_found(ApiErrorCode::LoadError, "user not found"))
					})?;

				let emote_set = if let Some(emote_set_id) = data.emote_set_id {
					// check if set exists
					let emote_set = global
						.emote_set_by_id_loader
						.load(emote_set_id.id())
						.await
						.map_err(|_| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load emote set",
							))
						})?
						.ok_or_else(|| {
							TransactionError::Custom(ApiError::not_found(ApiErrorCode::LoadError, "emote set not found"))
						})?;

					Some(emote_set)
				} else {
					None
				};

				let update_pull = data.unlink.is_some_and(|u| u).then_some(update::update! {
					#[query(pull)]
					User {
						connections: DbUserConnection {
							platform_id: id.clone(),
						}
					}
				});

				let Some(user) = tx
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
								search_updated_at: &None,
							},
							#[query(pull)]
							update_pull,
						},
						FindOneAndUpdateOptions::builder()
							.return_document(ReturnDocument::After)
							.build(),
					)
					.await?
				else {
					return Ok(None);
				};

				if let Some(true) = data.unlink {
					if user.connections.is_empty() {
						return Err(TransactionError::Custom(ApiError::bad_request(
							ApiErrorCode::BadRequest,
							"cannot remove last connection",
						)));
					}

					let connection = old_user
						.user
						.connections
						.into_iter()
						.find(|c| c.platform_id == id)
						.ok_or_else(|| {
							TransactionError::Custom(ApiError::not_found(ApiErrorCode::LoadError, "connection not found"))
						})?;

					tx.register_event(InternalEvent {
						actor: Some(authed_user.clone()),
						session_id: session.user_session_id(),
						data: InternalEventData::User {
							after: user.clone(),
							data: InternalEventUserData::RemoveConnection { connection },
						},
						timestamp: chrono::Utc::now(),
					})?;
				}

				if let Some(emote_set) = emote_set {
					let old = if let Some(set_id) = old_user.user.style.active_emote_set_id {
						global.emote_set_by_id_loader.load(set_id).await.map_err(|_| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load emote set",
							))
						})?
					} else {
						None
					};

					tx.register_event(InternalEvent {
						actor: Some(authed_user.clone()),
						session_id: session.user_session_id(),
						data: InternalEventData::User {
							after: user.clone(),
							data: InternalEventUserData::ChangeActiveEmoteSet {
								old: old.map(Box::new),
								new: Some(Box::new(emote_set)),
							},
						},
						timestamp: chrono::Utc::now(),
					})?;
				}

				Ok(Some(user))
			},
		)
		.await;

		match res {
			Ok(Some(user)) => {
				let full_user = global
					.user_loader
					.load_fast_user(global, user)
					.await
					.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?;

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
			Ok(None) => Ok(None),
			Err(TransactionError::Custom(e)) => Err(e),
			Err(e) => {
				tracing::error!(error = %e, "transaction failed");
				Err(ApiError::internal_server_error(
					ApiErrorCode::TransactionError,
					"transaction failed",
				))
			}
		}
	}

	#[graphql(guard = "RateLimitGuard::new(RateLimitResource::UserChangeEditor, 1)")]
	async fn editors(
		&self,
		ctx: &Context<'_>,
		editor_id: GqlObjectId,
		data: UserEditorUpdate,
	) -> Result<Option<Vec<Option<UserEditor>>>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let session = ctx
			.data::<Session>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing sesion data"))?;
		let authed_user = session.user()?;

		if !authed_user.has(UserPermission::InviteEditors) {
			return Err(ApiError::forbidden(
				ApiErrorCode::LackingPrivileges,
				"you are not allowed to invite editors",
			));
		}

		let permissions = data.permissions.unwrap_or(UserEditorModelPermission::none());

		// They should be able to remove themselves from the editor list
		if authed_user.id != self.id.id()
			&& !authed_user.has(UserPermission::ManageAny)
			&& (editor_id.id() != authed_user.id() || permissions.is_none())
		{
			let editor = global
				.user_editor_by_id_loader
				.load(UserEditorId {
					editor_id: authed_user.id,
					user_id: self.id.id(),
				})
				.await
				.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editor"))?
				.ok_or_else(|| {
					ApiError::forbidden(
						ApiErrorCode::LackingPrivileges,
						"you do not have permission to modify editors",
					)
				})?;

			if editor.state != UserEditorState::Accepted || !editor.permissions.has(EditorUserPermission::ManageEditors) {
				return Err(ApiError::forbidden(
					ApiErrorCode::LackingPrivileges,
					"you do not have permission to modify editors, you need the ManageEditors permission",
				));
			}
		}

		let res = transaction_with_mutex(
			global,
			Some(GeneralMutexKey::User(self.id.id()).into()),
			|mut tx| async move {
				let editor_id = UserEditorId {
					user_id: self.id.id(),
					editor_id: editor_id.id(),
				};

				if permissions.is_none() {
					// Remove editor
					let res = tx
						.find_one_and_delete(
							filter::filter! {
								DbUserEditor {
									#[query(rename = "_id", serde)]
									id: editor_id,
								}
							},
							None,
						)
						.await?;

					if let Some(editor) = res {
						let editor_user = global
							.user_loader
							.load_fast(global, editor.id.editor_id)
							.await
							.map_err(|_| {
								TransactionError::Custom(ApiError::internal_server_error(
									ApiErrorCode::LoadError,
									"failed to load user",
								))
							})?
							.ok_or_else(|| {
								TransactionError::Custom(ApiError::internal_server_error(
									ApiErrorCode::LoadError,
									"failed to load user",
								))
							})?;

						tx.register_event(InternalEvent {
							actor: Some(authed_user.clone()),
							session_id: session.user_session_id(),
							data: InternalEventData::UserEditor {
								after: editor,
								data: InternalEventUserEditorData::RemoveEditor {
									editor: Box::new(editor_user.user),
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					}
				} else {
					let old_permissions = tx
						.find_one(
							filter::filter! {
								DbUserEditor {
									#[query(rename = "_id", serde)]
									id: editor_id,
								}
							},
							None,
						)
						.await?
						.as_ref()
						.map(|e| e.permissions);

					// Add or update editor
					let permissions = permissions.to_db();

					if old_permissions == Some(permissions) {
						return Err(TransactionError::Custom(ApiError::bad_request(
							ApiErrorCode::BadRequest,
							"permissions are the same",
						)));
					}

					let now = chrono::Utc::now();

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
									permissions,
									updated_at: chrono::Utc::now(),
									search_updated_at: &None,
								},
								#[query(set_on_insert)]
								DbUserEditor {
									// TODO: Once the new website allows for pending editors, this should be changed to Pending
									#[query(serde)]
									state: UserEditorState::Accepted,
									notes: None,
									added_at: now,
									added_by_id: authed_user.id,
								}
							},
							FindOneAndUpdateOptions::builder()
								.upsert(true)
								.return_document(ReturnDocument::After)
								.build(),
						)
						.await?
						.ok_or_else(|| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load editor",
							))
						})?;

					// updated
					let editor_user = global
						.user_loader
						.load_fast(global, editor.id.editor_id)
						.await
						.map_err(|_| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load user",
							))
						})?
						.ok_or_else(|| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load user",
							))
						})?;

					if old_permissions.is_none() {
						tx.register_event(InternalEvent {
							actor: Some(authed_user.clone()),
							session_id: session.user_session_id(),
							data: InternalEventData::UserEditor {
								after: editor,
								data: InternalEventUserEditorData::AddEditor {
									editor: Box::new(editor_user.user),
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					} else {
						tx.register_event(InternalEvent {
							actor: Some(authed_user.clone()),
							session_id: session.user_session_id(),
							data: InternalEventData::UserEditor {
								after: editor,
								data: InternalEventUserEditorData::EditPermissions {
									old: old_permissions.unwrap_or_default(),
									editor: Box::new(editor_user.user),
								},
							},
							timestamp: chrono::Utc::now(),
						})?;
					}
				}

				Ok(())
			},
		)
		.await;

		match res {
			Ok(_) => {
				let editors = global
					.user_editor_by_user_id_loader
					.load(self.id.id())
					.await
					.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editors"))?
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
				Err(ApiError::internal_server_error(
					ApiErrorCode::TransactionError,
					"transaction failed",
				))
			}
		}
	}

	#[graphql(guard = "RateLimitGuard::new(RateLimitResource::UserChangeCosmetics, 1)")]
	async fn cosmetics<'ctx>(&self, ctx: &Context<'ctx>, update: UserCosmeticUpdate) -> Result<bool, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let session = ctx
			.data::<Session>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing sesion data"))?;
		let authed_user = session.user()?;

		if !update.selected {
			return Ok(true);
		}

		if authed_user.id != self.id.id() && !authed_user.has(UserPermission::ManageAny) {
			let editor = global
				.user_editor_by_id_loader
				.load(UserEditorId {
					editor_id: authed_user.id,
					user_id: self.id.id(),
				})
				.await
				.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load editor"))?
				.ok_or_else(|| {
					ApiError::forbidden(
						ApiErrorCode::LackingPrivileges,
						"you do not have permission to change this user's cosmetics",
					)
				})?;

			if editor.state != UserEditorState::Accepted || !editor.permissions.has(EditorUserPermission::ManageProfile) {
				return Err(ApiError::forbidden(
					ApiErrorCode::LackingPrivileges,
					"you do not have permission to modify this user's cosmetics, you need the ManageProfile permission",
				));
			}
		}

		let user = global
			.user_loader
			.load(global, self.id.id())
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?
			.ok_or_else(|| ApiError::not_found(ApiErrorCode::LoadError, "user not found"))?;

		let res =
			transaction_with_mutex(
				global,
				Some(GeneralMutexKey::User(self.id.id()).into()),
				|mut tx| async move {
					match update.kind {
				CosmeticKind::Paint => {
					let id: Option<PaintId> = if update.id.0.is_nil() { None } else { Some(update.id.id()) };

					// check if user has paint
					if id.is_some_and(|id| !user.computed.entitlements.paints.contains(&id)) {
						return Err(TransactionError::Custom(ApiError::forbidden(
							ApiErrorCode::LoadError,
							"you do not have permission to use this paint",
						)));
					}

					if user.style.active_paint_id == id {
						return Ok(true);
					}

					let new = if let Some(id) = id {
						Some(
							global
								.paint_by_id_loader
								.load(id)
								.await
								.map_err(|_| {
									TransactionError::Custom(ApiError::internal_server_error(
										ApiErrorCode::LoadError,
										"failed to load paint",
									))
								})?
								.ok_or_else(|| {
									TransactionError::Custom(ApiError::not_found(ApiErrorCode::LoadError, "paint not found"))
								})?,
						)
					} else {
						None
					};

					let old = if let Some(paint_id) = user.style.active_paint_id {
						global.paint_by_id_loader.load(paint_id).await.map_err(|_| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load badge",
							))
						})?
					} else {
						None
					};

					tx.register_event(InternalEvent {
						actor: Some(authed_user.clone()),
						session_id: session.user_session_id(),
						data: InternalEventData::User {
							after: user.user.clone(),
							data: InternalEventUserData::ChangeActivePaint {
								old: old.map(Box::new),
								new: new.map(Box::new),
							},
						},
						timestamp: chrono::Utc::now(),
					})?;

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
										active_paint_id: id,
									},
									updated_at: chrono::Utc::now(),
									search_updated_at: &None,
								}
							},
						)
						.await?;

					Ok(res.modified_count == 1)
				}
				CosmeticKind::Badge => {
					let id: Option<BadgeId> = if update.id.0.is_nil() { None } else { Some(update.id.id()) };

					// check if user has paint
					if id.is_some_and(|id| !user.computed.entitlements.badges.contains(&id)) {
						return Err(TransactionError::Custom(ApiError::forbidden(
							ApiErrorCode::LoadError,
							"you do not have permission to use this badge",
						)));
					}

					if user.style.active_badge_id == id {
						return Ok(true);
					}

					let new = if let Some(id) = id {
						Some(
							global
								.badge_by_id_loader
								.load(id)
								.await
								.map_err(|_| {
									TransactionError::Custom(ApiError::internal_server_error(
										ApiErrorCode::LoadError,
										"failed to load badge",
									))
								})?
								.ok_or_else(|| {
									TransactionError::Custom(ApiError::not_found(ApiErrorCode::LoadError, "badge not found"))
								})?,
						)
					} else {
						None
					};

					let old = if let Some(badge_id) = user.style.active_badge_id {
						global.badge_by_id_loader.load(badge_id).await.map_err(|_| {
							TransactionError::Custom(ApiError::internal_server_error(
								ApiErrorCode::LoadError,
								"failed to load badge",
							))
						})?
					} else {
						None
					};

					tx.register_event(InternalEvent {
						actor: Some(authed_user.clone()),
						session_id: session.user_session_id(),
						data: InternalEventData::User {
							after: user.user.clone(),
							data: InternalEventUserData::ChangeActiveBadge {
								old: old.map(Box::new),
								new: new.map(Box::new),
							},
						},
						timestamp: chrono::Utc::now(),
					})?;

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
										active_badge_id: id,
									},
									updated_at: chrono::Utc::now(),
									search_updated_at: &None,
								},
							},
						)
						.await?;

					Ok(res.modified_count == 1)
				}
				CosmeticKind::Avatar => Err(TransactionError::Custom(ApiError::not_implemented(
					ApiErrorCode::BadRequest,
					"avatar cosmetics mutations are not supported via this endpoint, use the upload endpoint instead",
				))),
			}
				},
			)
			.await;

		match res {
			Ok(b) => Ok(b),
			Err(TransactionError::Custom(e)) => Err(e),
			Err(e) => {
				tracing::error!(error = %e, "transaction failed");
				Err(ApiError::internal_server_error(
					ApiErrorCode::TransactionError,
					"transaction failed",
				))
			}
		}
	}

	#[graphql(guard = "PermissionGuard::one(RolePermission::Assign)")]
	async fn roles<'ctx>(
		&self,
		ctx: &Context<'ctx>,
		role_id: GqlObjectId,
		action: ListItemAction,
	) -> Result<Vec<GqlObjectId>, ApiError> {
		let global: &Arc<Global> = ctx
			.data()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing global data"))?;
		let session = ctx
			.data::<Session>()
			.map_err(|_| ApiError::internal_server_error(ApiErrorCode::MissingContext, "missing sesion data"))?;
		let authed_user = session.user()?;

		let role = global
			.role_by_id_loader
			.load(role_id.id())
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load role"))?
			.ok_or_else(|| ApiError::not_found(ApiErrorCode::LoadError, "role not found"))?;

		if !authed_user.computed.permissions.is_superset_of(&role.permissions) {
			return Err(ApiError::forbidden(
				ApiErrorCode::LackingPrivileges,
				"the role has a higher permission level than you",
			));
		}

		let target_user = global
			.user_loader
			.load(global, self.id.id())
			.await
			.map_err(|()| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load user"))?
			.ok_or_else(|| ApiError::not_found(ApiErrorCode::LoadError, "user not found"))?;

		let res = transaction_with_mutex(
			global,
			Some(GeneralMutexKey::User(self.id.id()).into()),
			|mut tx| async move {
				let roles = match action {
					ListItemAction::Add => {
						let edge_id = EntitlementEdgeId {
							from: EntitlementEdgeKind::User { user_id: self.id.id() },
							to: EntitlementEdgeKind::Role { role_id: role_id.id() },
							managed_by: None,
						};

						let res = tx
							.update_one(
								filter::filter! {
									EntitlementEdge {
										#[query(rename = "_id", serde)]
										id: &edge_id
									}
								},
								update::update! {
									#[query(set_on_insert)]
									EntitlementEdge {
										#[query(serde, rename = "_id")]
										id: edge_id,
									}
								},
								Some(UpdateOptions::builder().upsert(true).build()),
							)
							.await?;

						if res.upserted_id.is_some() {
							tx.register_event(InternalEvent {
								actor: Some(authed_user.clone()),
								session_id: session.user_session_id(),
								data: InternalEventData::User {
									after: target_user.user.clone(),
									data: InternalEventUserData::AddEntitlement {
										target: EntitlementEdgeKind::Role { role_id: role_id.id() },
									},
								},
								timestamp: chrono::Utc::now(),
							})?;
						}

						let no_role = !target_user.computed.roles.contains(&role_id.id());

						target_user
							.computed
							.entitlements
							.roles
							.iter()
							.copied()
							// If the user didnt have the role before, we add it
							.chain(no_role.then_some(role_id.id()))
							.map(Into::into)
							.collect()
					}
					ListItemAction::Remove => {
						if tx
							.delete_one(
								filter::filter! {
									EntitlementEdge {
										#[query(rename = "_id", serde)]
										id: EntitlementEdgeId {
											from: EntitlementEdgeKind::User { user_id: self.id.id() },
											to: EntitlementEdgeKind::Role { role_id: role_id.id() },
											managed_by: None,
										}
									}
								},
								None,
							)
							.await?
							.deleted_count == 1
						{
							tx.register_event(InternalEvent {
								actor: Some(authed_user.clone()),
								session_id: session.user_session_id(),
								data: InternalEventData::User {
									after: target_user.user.clone(),
									data: InternalEventUserData::RemoveEntitlement {
										target: EntitlementEdgeKind::Role { role_id: role_id.id() },
									},
								},
								timestamp: chrono::Utc::now(),
							})?;
						}

						// They might have the role via some other entitlement.
						let role_via_edge = target_user.computed.raw_entitlements.iter().flat_map(|e| e.iter()).any(|e| {
							e.id.to == EntitlementEdgeKind::Role { role_id: role_id.id() }
								&& (e.id.from != EntitlementEdgeKind::User { user_id: self.id.id() }
									|| e.id.managed_by.is_some())
						});

						target_user
							.computed
							.entitlements
							.roles
							.iter()
							.copied()
							.filter(|id| role_via_edge || *id != role_id.id())
							.map(Into::into)
							.collect()
					}
					ListItemAction::Update => {
						return Err(TransactionError::Custom(ApiError::not_implemented(
							ApiErrorCode::BadRequest,
							"update role is not implemented",
						)));
					}
				};

				Ok(roles)
			},
		)
		.await;

		match res {
			Ok(roles) => Ok(roles),
			Err(TransactionError::Custom(e)) => Err(e),
			Err(e) => {
				tracing::error!(error = %e, "transaction failed");
				Err(ApiError::internal_server_error(
					ApiErrorCode::TransactionError,
					"transaction failed",
				))
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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use scuffle_foundations::batcher::dataloader::{DataLoader, Loader, LoaderOutput};
use scuffle_foundations::batcher::BatcherConfig;
use scuffle_foundations::telemetry::opentelemetry::OpenTelemetrySpanExt;
use shared::database::entitlement::{CalculatedEntitlements, EntitlementEdgeKind};
use shared::database::entitlement_edge::EntitlementEdgeGraphTraverse;
use shared::database::graph::{Direction, GraphTraverse};
use shared::database::loader::dataloader::BatchLoad;
use shared::database::role::permissions::{Permissions, PermissionsExt, UserPermission};
use shared::database::role::{Role, RoleId};
use shared::database::user::ban::ActiveBans;
use shared::database::user::{FullUser, User, UserComputed, UserId};

use crate::global::Global;

pub struct FullUserLoader {
	pub computed_loader: DataLoader<UserComputedLoader>,
}

impl FullUserLoader {
	pub fn new(global: Weak<Global>) -> Self {
		Self {
			computed_loader: UserComputedLoader::new(global.clone()),
		}
	}

	/// Performs a full user load fetching all necessary data using the graph
	pub async fn load(&self, global: &Arc<Global>, user_id: UserId) -> Result<Option<FullUser>, ()> {
		self.load_many(global, std::iter::once(user_id))
			.await
			.map(|mut users| users.remove(&user_id))
	}

	/// Performs a full user load fetching all necessary data using the graph
	pub async fn load_many(
		&self,
		global: &Arc<Global>,
		user_ids: impl IntoIterator<Item = UserId>,
	) -> Result<HashMap<UserId, FullUser>, ()> {
		let users = global.user_by_id_loader.load_many(user_ids).await?;
		self.load_user_many(global, users.into_values()).await
	}

	/// Performs a full user load fetching all necessary data using the graph
	pub async fn load_user(&self, global: &Arc<Global>, user: User) -> Result<FullUser, ()> {
		let id = user.id;
		self.load_user_many(global, std::iter::once(user))
			.await?
			.remove(&id)
			.ok_or(())
	}

	/// Performs a full user load fetching all necessary data using the graph
	pub async fn load_user_many(
		&self,
		global: &Arc<Global>,
		user: impl IntoIterator<Item = User>,
	) -> Result<HashMap<UserId, FullUser>, ()> {
		let users = user.into_iter().collect::<Vec<_>>();

		let computed = self.computed_loader.load_many(users.iter().map(|user| user.id)).await?;

		let bans = global
			.user_ban_by_user_id_loader
			.load_many(
				users
					.iter()
					.filter_map(|user| if user.has_bans { Some(user.id) } else { None }),
			)
			.await?;

		let profile_pictures = global
			.user_profile_picture_id_loader
			.load_many(users.iter().filter_map(|user| {
				let computed = computed.get(&user.id)?;

				if computed.permissions.has(UserPermission::UseCustomProfilePicture) {
					user.style.active_profile_picture
				} else {
					None
				}
			}))
			.await?;

		Ok(users
			.into_iter()
			.filter_map(|mut user| {
				let mut computed = computed.get(&user.id)?.clone();

				if let Some(active_bans) = bans.get(&user.id).and_then(|bans| ActiveBans::new(bans)) {
					computed.permissions.merge(active_bans.permissions());
				}

				let active_profile_picture = user
					.style
					.active_profile_picture
					.and_then(|id| profile_pictures.get(&id).cloned());

				user.style.active_badge_id = user.style.active_badge_id.and_then(|id| {
					if computed.permissions.has(UserPermission::UseBadge) && computed.entitlements.badges.contains(&id) {
						Some(id)
					} else {
						None
					}
				});

				user.style.active_paint_id = user.style.active_paint_id.and_then(|id| {
					if computed.permissions.has(UserPermission::UsePaint) && computed.entitlements.paints.contains(&id) {
						Some(id)
					} else {
						None
					}
				});

				Some((
					user.id,
					FullUser {
						user,
						computed,
						active_profile_picture,
					},
				))
			})
			.collect())
	}

	/// Performs a fast user load fetching using the cache'ed data
	pub async fn load_fast(&self, global: &Arc<Global>, user_id: UserId) -> Result<Option<FullUser>, ()> {
		self.load_fast_many(global, std::iter::once(user_id))
			.await
			.map(|mut users| users.remove(&user_id))
	}

	/// Performs a fast user load fetching using the cache'ed data
	pub async fn load_fast_many(
		&self,
		global: &Arc<Global>,
		user_ids: impl IntoIterator<Item = UserId>,
	) -> Result<HashMap<UserId, FullUser>, ()> {
		let users = global.user_by_id_loader.load_many(user_ids).await?;
		self.load_fast_user_many(global, users.into_values()).await
	}

	/// Performs a fast user load fetching using the cache'ed data
	pub async fn load_fast_user(&self, global: &Arc<Global>, user: User) -> Result<FullUser, ()> {
		let id = user.id;
		self.load_fast_user_many(global, std::iter::once(user))
			.await?
			.remove(&id)
			.ok_or(())
	}

	/// Performs a fast user load fetching using the cache'ed data
	pub async fn load_fast_user_many(
		&self,
		global: &Arc<Global>,
		user: impl IntoIterator<Item = User>,
	) -> Result<HashMap<UserId, FullUser>, ()> {
		let mut role_ids = HashSet::new();

		let mut users = user
			.into_iter()
			.map(|user| {
				let id = user.id;
				let computed = UserComputed {
					permissions: Permissions::default(),
					entitlements: CalculatedEntitlements::new(user.cached.entitlements.iter().cloned()),
					highest_role_rank: -1,
					highest_role_color: None,
					raw_entitlements: None,
					roles: vec![],
				};

				role_ids.extend(computed.entitlements.roles.iter().cloned());

				(
					id,
					FullUser {
						user,
						computed,
						active_profile_picture: None,
					},
				)
			})
			.collect::<HashMap<_, _>>();

		let mut roles: Vec<_> = global
			.role_by_id_loader
			.load_many(role_ids.iter().copied())
			.await?
			.into_values()
			.collect();

		let bans = global
			.user_ban_by_user_id_loader
			.load_many(
				users
					.values()
					.filter_map(|user| if user.has_bans { Some(user.id) } else { None }),
			)
			.await?;

		roles.sort_by_key(|r| r.rank);

		for user in users.values_mut() {
			user.computed.permissions = compute_permissions(&roles, &user.computed.entitlements.roles);
			if let Some(active_bans) = bans.get(&user.id).and_then(|bans| ActiveBans::new(bans)) {
				user.computed.permissions.merge(active_bans.permissions());
			}
		}

		let profile_pictures = global
			.user_profile_picture_id_loader
			.load_many(users.values().filter_map(|user| {
				if user.computed.permissions.has(UserPermission::UseCustomProfilePicture) {
					user.style.active_profile_picture
				} else {
					None
				}
			}))
			.await?;

		for user in users.values_mut() {
			user.computed.highest_role_rank = compute_highest_role_rank(&roles, &user.computed.entitlements.roles);
			user.computed.highest_role_color = compute_highest_role_color(&roles, &user.computed.entitlements.roles);
			user.computed.roles = roles
				.iter()
				.map(|r| r.id)
				.filter(|r| user.computed.entitlements.roles.contains(r))
				.collect();

			user.active_profile_picture = user
				.style
				.active_profile_picture
				.and_then(|id| profile_pictures.get(&id).cloned());

			user.user.style.active_badge_id = user.style.active_badge_id.and_then(|id| {
				if user.computed.permissions.has(UserPermission::UseBadge) && user.computed.entitlements.badges.contains(&id)
				{
					Some(id)
				} else {
					None
				}
			});

			user.user.style.active_paint_id = user.style.active_paint_id.and_then(|id| {
				if user.computed.permissions.has(UserPermission::UsePaint) && user.computed.entitlements.paints.contains(&id)
				{
					Some(id)
				} else {
					None
				}
			});
		}

		Ok(users)
	}
}

pub struct UserComputedLoader {
	global: Weak<Global>,
	config: BatcherConfig,
}

impl UserComputedLoader {
	pub fn new(global: Weak<Global>) -> DataLoader<Self> {
		Self::new_with_config(
			global,
			BatcherConfig {
				name: "UserComputedLoader".to_string(),
				concurrency: 500,
				max_batch_size: 1000,
				sleep_duration: std::time::Duration::from_millis(20),
			},
		)
	}

	pub fn new_with_config(global: Weak<Global>, config: BatcherConfig) -> DataLoader<Self> {
		DataLoader::new(Self { global, config })
	}
}

impl Loader for UserComputedLoader {
	type Key = UserId;
	type Value = UserComputed;

	fn config(&self) -> BatcherConfig {
		self.config.clone()
	}

	#[tracing::instrument(skip_all, fields(key_count = keys.len()))]
	async fn load(&self, keys: Vec<Self::Key>) -> LoaderOutput<Self> {
		tracing::Span::current().make_root();

		let _batch = BatchLoad::new(&self.config.name, keys.len());

		let global = &self.global.upgrade().ok_or(())?;

		let traverse = &EntitlementEdgeGraphTraverse {
			inbound_loader: &global.entitlement_edge_inbound_loader,
			outbound_loader: &global.entitlement_edge_outbound_loader,
		};

		let result = futures::future::try_join_all(keys.into_iter().map(|user_id| async move {
			let raw_entitlements = traverse
				.traversal(
					Direction::Outbound,
					std::iter::once(EntitlementEdgeKind::GlobalDefaultEntitlementGroup)
						.chain((!user_id.is_nil()).then_some(EntitlementEdgeKind::User { user_id })),
				)
				.await?;

			Result::<_, ()>::Ok((user_id, raw_entitlements))
		}))
		.await?;

		let mut role_ids = HashSet::new();

		let mut result = result
			.into_iter()
			.map(|(id, raw_entitlements)| {
				let entitlements = CalculatedEntitlements::new(raw_entitlements.iter().map(|e| e.id.to.clone()));

				role_ids.extend(entitlements.roles.iter().cloned());

				(
					id,
					UserComputed {
						permissions: Permissions::default(),
						entitlements,
						highest_role_rank: -1,
						highest_role_color: None,
						roles: vec![],
						raw_entitlements: Some(raw_entitlements),
					},
				)
			})
			.collect::<HashMap<_, _>>();

		let mut roles: Vec<_> = global
			.role_by_id_loader
			.load_many(role_ids.into_iter())
			.await?
			.into_values()
			.collect();
		roles.sort_by_key(|r| r.rank);

		for user in result.values_mut() {
			user.permissions = compute_permissions(&roles, &user.entitlements.roles);
			user.highest_role_rank = compute_highest_role_rank(&roles, &user.entitlements.roles);
			user.highest_role_color = compute_highest_role_color(&roles, &user.entitlements.roles);
			user.roles = roles
				.iter()
				.map(|r| r.id)
				.filter(|r| user.entitlements.roles.contains(r))
				.collect();
		}

		Ok(result)
	}
}

fn compute_permissions(sorted_roles: &[Role], user_roles: &HashSet<RoleId>) -> Permissions {
	sorted_roles
		.iter()
		.filter(|role| user_roles.contains(&role.id))
		.map(|role| &role.permissions)
		.fold(Permissions::default(), |mut acc, p| {
			acc.merge_ref(p);
			acc
		})
}

fn compute_highest_role_rank(sorted_roles: &[Role], user_roles: &HashSet<RoleId>) -> i32 {
	sorted_roles
		.iter()
		.rev()
		.find_map(|role| {
			if user_roles.contains(&role.id) {
				Some(role.rank)
			} else {
				None
			}
		})
		.unwrap_or(-1)
}

fn compute_highest_role_color(sorted_roles: &[Role], user_roles: &HashSet<RoleId>) -> Option<i32> {
	sorted_roles
		.iter()
		.rev()
		.filter(|role| user_roles.contains(&role.id))
		.find_map(|role| role.color)
}

use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;

use futures::TryStreamExt;
use shared::database::duration::DurationUnit;
use shared::database::emote_set::{EmoteSet, EmoteSetId, EmoteSetKind};
use shared::database::entitlement::{EntitlementEdge, EntitlementEdgeId, EntitlementEdgeKind, EntitlementEdgeManagedBy};
use shared::database::product::subscription::{Subscription, SubscriptionId, SubscriptionPeriod, SubscriptionState};
use shared::database::product::SubscriptionBenefitCondition;
use shared::database::queries::{filter, update};
use shared::database::MongoCollection;
use shared::event::{InternalEvent, InternalEventData, InternalEventEmoteSetData};

use crate::global::Global;
use crate::http::error::{ApiError, ApiErrorCode};
use crate::transactions::{transaction_with_mutex, TransactionError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAge {
	pub extra: chrono::Duration,
	pub months: i32,
	pub days: i32,
	pub periods: Vec<StartEnd>,
	pub expected_end: chrono::DateTime<chrono::Utc>,
}

impl SubAge {
	pub fn new(periods: &[SubscriptionPeriod]) -> Self {
		// We need to sum up all the time so that we can calculate the age of the
		// subscription. We want to make sure there are no overlapping periods so we
		// dont have duplicate time.
		let now = chrono::Utc::now();

		let expected_end = periods.iter().map(|p| p.end).max().unwrap_or(now);

		let mut combined_periods = periods
			.iter()
			.map(|p| StartEnd {
				start: p.start,
				end: p.end.min(now),
			})
			.collect::<Vec<_>>();

		combined_periods.sort_by(|a, b| a.start.cmp(&b.start));

		let merged_periods: Vec<StartEnd> = combined_periods.into_iter().fold(Vec::new(), |mut acc, period| {
			if acc.is_empty() {
				acc.push(period);
				return acc;
			}

			let last = acc.last_mut().unwrap();
			if last.end >= period.start {
				last.end = period.end.max(last.end);
			} else {
				acc.push(period);
			}

			acc
		});

		let days = merged_periods
			.iter()
			.map(|p| (p.end.min(now) - p.start))
			.sum::<chrono::Duration>()
			.num_days() as i32;

		let months = days as f64 / (365.25 / 12.0);
		let extra = chrono::Duration::days((months.fract() * 30.44).round() as i64);
		let months = months as i32;

		SubAge {
			extra,
			months,
			days,
			periods: merged_periods,
			expected_end,
		}
	}

	pub fn meets_condition(&self, condition: &SubscriptionBenefitCondition) -> bool {
		// Consider the Subscription, if their sub is set to end in the future then they
		// should get the entitlements for the period that they are currently in.
		// if you sub to twitch you are given the 1 month sub badge even though you
		// havent subbed for the entire month yet, this is because the sub is set to end
		// in the future. However if you unsub at the end of your term you would have
		// completed the month and wouldnt get the next badge because your sub has
		// ended. Then once you start subbing again you would get the next badge.
		let next_period = if self.expected_end > chrono::Utc::now() { 1 } else { 0 };

		match condition {
			SubscriptionBenefitCondition::Duration(DurationUnit::Days(d)) => self.days + next_period >= *d,
			SubscriptionBenefitCondition::Duration(DurationUnit::Months(m)) => self.months + next_period >= *m,
			SubscriptionBenefitCondition::TimePeriod(tp) => self.periods.iter().any(|p| {
				(p.start <= tp.start && p.end >= tp.start)
					|| (p.start <= tp.end && p.end >= tp.end)
					|| (p.start >= tp.start && p.end <= tp.end)
			}),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartEnd {
	pub start: chrono::DateTime<chrono::Utc>,
	pub end: chrono::DateTime<chrono::Utc>,
}

/// Grants entitlements for a subscription.
pub async fn refresh(global: &Arc<Global>, subscription_id: SubscriptionId) -> Result<(), ApiError> {
	let product = global
		.subscription_product_by_id_loader
		.load(subscription_id.product_id)
		.await
		.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load product"))?
		.ok_or_else(|| ApiError::internal_server_error(ApiErrorCode::LoadError, "product not found"))?;

	// load existing edges
	let outgoing: HashSet<_> = global
		.entitlement_edge_outbound_loader
		.load(EntitlementEdgeKind::Subscription { subscription_id })
		.await
		.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription entitlements"))?
		.unwrap_or_default()
		.into_iter()
		.map(|e| e.id.to)
		.collect();

	let incoming: HashSet<_> = global
		.entitlement_edge_inbound_loader
		.load(EntitlementEdgeKind::Subscription { subscription_id })
		.await
		.map_err(|_| ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription entitlements"))?
		.unwrap_or_default()
		.into_iter()
		.map(|e| e.id.from)
		.collect();

	// load all periods
	let periods: Vec<_> = SubscriptionPeriod::collection(&global.db)
		.find(filter::filter! {
			SubscriptionPeriod {
				#[query(serde)]
				subscription_id,
			}
		})
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to load subscription periods");
			ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to load subscription periods")
		})?
		.try_collect()
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to collect subscription periods");
			ApiError::internal_server_error(ApiErrorCode::LoadError, "failed to collect subscription periods")
		})?;

	let mut new_edges = vec![];
	let mut remove_edges = vec![];
	let sub_age = SubAge::new(&periods);

	for benefit in product.benefits {
		let is_fulfilled = sub_age.meets_condition(&benefit.condition);

		let benefit_edge = EntitlementEdgeId {
			from: EntitlementEdgeKind::Subscription { subscription_id },
			to: EntitlementEdgeKind::SubscriptionBenefit {
				subscription_benefit_id: benefit.id,
			},
			managed_by: Some(EntitlementEdgeManagedBy::Subscription { subscription_id }),
		};

		if is_fulfilled && !outgoing.contains(&benefit_edge.to) {
			new_edges.push(benefit_edge);
		} else if !is_fulfilled && outgoing.contains(&benefit_edge.to) {
			remove_edges.push(benefit_edge);
		}
	}

	let now = chrono::Utc::now();
	let active_periods = periods.iter().filter(|p| p.start < now && p.end > now).collect::<Vec<_>>();

	let user_edge = EntitlementEdgeId {
		from: EntitlementEdgeKind::User {
			user_id: subscription_id.user_id,
		},
		to: EntitlementEdgeKind::Subscription { subscription_id },
		managed_by: Some(EntitlementEdgeManagedBy::Subscription { subscription_id }),
	};

	if !active_periods.is_empty() {
		if !incoming.contains(&user_edge.from) {
			new_edges.push(user_edge);
		}

		let state = if active_periods.iter().any(|period| period.auto_renew) {
			SubscriptionState::Active
		} else {
			SubscriptionState::CancelAtEnd
		};

		Subscription::collection(&global.db)
			.update_one(
				filter::filter! {
					Subscription {
						#[query(rename = "_id", serde)]
						id: subscription_id,
					}
				},
				update::update! {
					#[query(set)]
					Subscription {
						#[query(serde)]
						state,
						ended_at: &None,
						updated_at: chrono::Utc::now(),
						search_updated_at: &None,
					},
					#[query(set_on_insert)]
					Subscription {
						#[query(rename = "_id", serde)]
						id: subscription_id,
						created_at: chrono::Utc::now(),
					}
				},
			)
			.upsert(true)
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to update subscription");
				ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to update subscription")
			})?;
	} else {
		if incoming.contains(&user_edge.from) {
			remove_edges.push(user_edge);
		}

		Subscription::collection(&global.db)
			.update_one(
				filter::filter! {
					Subscription {
						#[query(rename = "_id", serde)]
						id: subscription_id,
					}
				},
				update::update! {
					#[query(set)]
					Subscription {
						#[query(serde)]
						state: SubscriptionState::Ended,
						ended_at: Some(sub_age.expected_end),
						updated_at: chrono::Utc::now(),
						search_updated_at: &None,
					},
					#[query(set_on_insert)]
					Subscription {
						#[query(rename = "_id", serde)]
						id: subscription_id,
						created_at: chrono::Utc::now(),
					}
				},
			)
			.upsert(true)
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to update subscription");
				ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to update subscription")
			})?;
	}

	if !remove_edges.is_empty() {
		EntitlementEdge::collection(&global.db)
			.delete_many(filter::filter! {
				EntitlementEdge {
					#[query(rename = "_id", selector = "in", serde)]
					id: remove_edges,
				}
			})
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to delete entitlement edges");
				ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to delete entitlement edges")
			})?;
	}

	if !new_edges.is_empty() {
		EntitlementEdge::collection(&global.db)
			.insert_many(new_edges.into_iter().map(|id| EntitlementEdge { id }))
			.await
			.map_err(|e| {
				tracing::error!(error = %e, "failed to insert entitlement edges");
				ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to insert entitlement edges")
			})?;
	}

	if EmoteSet::collection(&global.db)
		.find_one(filter::filter! {
			EmoteSet {
				owner_id: subscription_id.user_id,
				#[query(serde)]
				kind: EmoteSetKind::Personal,
			}
		})
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to update emote set");
			ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to update emote set")
		})?
		.is_none()
	{
		transaction_with_mutex(
			global,
			Some(format!("mutex:user:sub:personal:{}", subscription_id.user_id).into()),
			|mut tx| async move {
				if tx
					.find_one(
						filter::filter! {
							EmoteSet {
								owner_id: subscription_id.user_id,
								#[query(serde)]
								kind: EmoteSetKind::Personal,
							}
						},
						None,
					)
					.await?
					.is_some()
				{
					return Ok(());
				}

				let set = EmoteSet {
					id: EmoteSetId::new(),
					name: "Personal Emote Set".to_string(),
					owner_id: Some(subscription_id.user_id),
					kind: EmoteSetKind::Personal,
					updated_at: chrono::Utc::now(),
					origin_config: None,
					capacity: Some(5), // TODO: this is hard coded however we should likely get this from the sub product
					description: None,
					emotes: vec![],
					emotes_changed_since_reindex: false,
					tags: vec![],
					search_updated_at: None,
				};

				tx.insert_one::<EmoteSet>(&set, None).await?;

				tx.register_event(InternalEvent {
					actor: None,
					session_id: None,
					timestamp: chrono::Utc::now(),
					data: InternalEventData::EmoteSet {
						after: set,
						data: InternalEventEmoteSetData::Create,
					},
				})?;

				Ok::<_, TransactionError<Infallible>>(())
			},
		)
		.await
		.map_err(|e| {
			tracing::error!(error = %e, "failed to create personal emote set");
			ApiError::internal_server_error(ApiErrorCode::MutationError, "failed to create personal emote set")
		})?;
	}

	Ok(())
}

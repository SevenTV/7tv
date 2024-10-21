use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::TryStreamExt;
use mongodb::error::{TRANSIENT_TRANSACTION_ERROR, UNKNOWN_TRANSACTION_COMMIT_RESULT};
use mongodb::results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult};
use shared::database::queries::{filter, update};
use shared::database::stored_event::StoredEvent;
use shared::database::MongoCollection;
use shared::event::{InternalEvent, InternalEventPayload};
use spin::Mutex;

use crate::global::Global;

pub struct TransactionSession<'a, E>(Arc<Mutex<SessionInner<'a>>>, PhantomData<E>);

impl<'a, E: Debug> TransactionSession<'a, E> {
	fn new(inner: Arc<Mutex<SessionInner<'a>>>) -> Self {
		Self(inner, PhantomData)
	}

	async fn reset(&mut self) -> Result<(), TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;
		this.events.clear();
		this.session.start_transaction().await?;
		Ok(())
	}

	fn clone(&self) -> Self {
		Self(self.0.clone(), PhantomData)
	}
}

impl<E: Debug> TransactionSession<'_, E> {
	#[allow(unused)]
	pub async fn find<U: MongoCollection + serde::de::DeserializeOwned>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::FindOptions>>,
	) -> Result<Vec<U>, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let mut find = U::collection(&this.global.db)
			.find(filter)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(find.stream(&mut this.session).try_collect().await?)
	}

	#[allow(unused)]
	pub async fn find_one<U: MongoCollection + serde::de::DeserializeOwned>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::FindOneOptions>>,
	) -> Result<Option<U>, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.find_one(filter)
			.with_options(options)
			.session(&mut this.session)
			.await
			.map_err(TransactionError::Mongo)?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn find_one_and_update<U: MongoCollection + serde::de::DeserializeOwned>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		update: impl Into<update::Update<U>>,
		options: impl Into<Option<mongodb::options::FindOneAndUpdateOptions>>,
	) -> Result<Option<U>, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.find_one_and_update(filter, update)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn find_one_and_delete<U: MongoCollection + serde::de::DeserializeOwned>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::FindOneAndDeleteOptions>>,
	) -> Result<Option<U>, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.find_one_and_delete(filter)
			.with_options(options)
			.session(&mut this.session)
			.await
			.map_err(TransactionError::Mongo)?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn update<U: MongoCollection>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		update: impl Into<update::Update<U>>,
		options: impl Into<Option<mongodb::options::UpdateOptions>>,
	) -> Result<UpdateResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.update_many(filter, update)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn update_one<U: MongoCollection>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		update: impl Into<update::Update<U>>,
		options: impl Into<Option<mongodb::options::UpdateOptions>>,
	) -> Result<UpdateResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.update_one(filter, update)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn delete<U: MongoCollection>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::DeleteOptions>>,
	) -> Result<DeleteResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.delete_many(filter)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn delete_one<U: MongoCollection>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::DeleteOptions>>,
	) -> Result<DeleteResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.delete_one(filter)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn count<U: MongoCollection>(
		&mut self,
		filter: impl Into<filter::Filter<U>>,
		options: impl Into<Option<mongodb::options::CountOptions>>,
	) -> Result<u64, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.count_documents(filter)
			.with_options(options)
			.session(&mut this.session)
			.await
			.map_err(TransactionError::Mongo)?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn insert_one<U: MongoCollection + serde::Serialize>(
		&mut self,
		insert: impl Borrow<U>,
		options: impl Into<Option<mongodb::options::InsertOneOptions>>,
	) -> Result<InsertOneResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.insert_one(insert)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	#[allow(unused)]
	pub async fn insert_many<U: MongoCollection + serde::Serialize>(
		&mut self,
		items: impl IntoIterator<Item = impl Borrow<U>>,
		options: impl Into<Option<mongodb::options::InsertManyOptions>>,
	) -> Result<InsertManyResult, TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;

		let result = U::collection(&this.global.db)
			.insert_many(items)
			.with_options(options)
			.session(&mut this.session)
			.await?;

		Ok(result)
	}

	pub fn register_event(&mut self, event: InternalEvent) -> Result<(), TransactionError<E>> {
		let mut this = self.0.try_lock().ok_or(TransactionError::SessionLocked)?;
		this.events.push(event);
		Ok(())
	}
}

struct SessionInner<'a> {
	global: &'a Arc<Global>,
	session: mongodb::ClientSession,
	events: Vec<InternalEvent>,
}

#[derive(thiserror::Error, Debug)]
pub enum TransactionError<E: Debug> {
	#[error("mongo error: {0}")]
	Mongo(#[from] mongodb::error::Error),
	#[error("session locked after returning")]
	SessionLocked,
	#[error("event serialize error: {0}")]
	EventSerialize(#[from] rmp_serde::encode::Error),
	#[error("event publish error: {0}")]
	EventPublish(#[from] async_nats::PublishError),
	#[error("custom error: {0:?}")]
	Custom(E),
	#[error("too many failures")]
	TooManyFailures,
}

pub type TransactionResult<T, E> = Result<T, TransactionError<E>>;

pub async fn with_transaction<'a, T, E, F, Fut>(global: &'a Arc<Global>, f: F) -> TransactionResult<T, E>
where
	F: FnOnce(TransactionSession<'a, E>) -> Fut + Clone + 'a,
	Fut: std::future::Future<Output = TransactionResult<T, E>> + 'a,
	E: Debug,
{
	let session = global.mongo.start_session().await?;

	let mut session = TransactionSession::new(Arc::new(Mutex::new(SessionInner {
		global,
		session,
		events: Vec::new(),
	})));

	let mut retry_count = 0;

	'retry_operation: loop {
		if retry_count > 3 {
			return Err(TransactionError::TooManyFailures);
		}

		retry_count += 1;
		session.reset().await?;
		let result = (f.clone())(session.clone()).await;
		let mut session_inner = session.0.try_lock().ok_or(TransactionError::SessionLocked)?;
		match result {
			Ok(output) => 'retry_commit: loop {
				let events = session_inner
					.events
					.iter()
					.cloned()
					.filter_map(|e| StoredEvent::try_from(e).ok())
					.collect::<Vec<_>>();

				if !events.is_empty() {
					StoredEvent::collection(&global.db)
						.insert_many(events)
						.session(&mut session_inner.session)
						.await?;
				}

				match session_inner.session.commit_transaction().await {
					Ok(_) => {
						let payload = InternalEventPayload::new(session_inner.events.drain(..));
						let payload = rmp_serde::to_vec_named(&payload)?;

						global.nats.publish("api.v4.events", payload.into()).await?;

						return Ok(output);
					}
					Err(err) => {
						tracing::warn!(error = %err, "transaction commit error");

						if err.contains_label(UNKNOWN_TRANSACTION_COMMIT_RESULT) {
							continue 'retry_commit;
						} else if err.contains_label(TRANSIENT_TRANSACTION_ERROR) {
							continue 'retry_operation;
						}

						return Err(TransactionError::Mongo(err));
					}
				}
			},
			Err(err) => {
				if let TransactionError::Mongo(err) = &err {
					if err.contains_label(TRANSIENT_TRANSACTION_ERROR) {
						tracing::warn!(error = %err, "transaction error");
						continue 'retry_operation;
					}
				}

				session_inner.session.abort_transaction().await?;

				return Err(err);
			}
		}
	}
}

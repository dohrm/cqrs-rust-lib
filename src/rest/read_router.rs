use crate::read::Storage;
use crate::Aggregate;
use std::sync::Arc;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

#[derive(Clone)]
pub struct CQRSReadRouter<A, S>
where
    A: Aggregate + ToSchema,
    S: Storage<A>,
{
    _phantom: std::marker::PhantomData<A>,
    storage: Arc<S>,
}

impl<A, S> CQRSReadRouter<A, S>
where
    A: Aggregate + ToSchema + 'static,
    S: Storage<A> + 'static,
{
    #[must_use]
    fn new(storage: Arc<S>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            storage,
        }
    }

    pub fn routes(storage: Arc<S>) -> OpenApiRouter {
        let state = Self::new(storage);
        let mut schemas = vec![];
        A::schemas(&mut schemas);
        S::Query::schemas(&mut schemas);
        S::View::schemas(&mut schemas);

        let mut result = OpenApiRouter::<CQRSReadRouter<A, S>>::new();

        result.with_state(state)
    }
}

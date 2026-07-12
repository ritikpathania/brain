use futures_util::future::BoxFuture;

/// Semantic capability contract interface with strongly typed inputs and outputs.
pub trait Capability<Target, Context, Error>: Send + Sync {
    /// Strongly-typed request payload DTO.
    type Request: serde::de::DeserializeOwned + Send;
    /// Strongly-typed response payload DTO.
    type Response: serde::Serialize + Send;

    /// Unique capability identifier name.
    fn name(&self) -> &'static str;
    /// Informative description of the capability.
    fn description(&self) -> &'static str;
    /// JSON Schema of the expected input payload.
    fn input_schema(&self) -> serde_json::Value;

    /// Executes the capability inside the context of a Target.
    fn execute<'a>(
        &'a self,
        target: &'a Target,
        req: Self::Request,
        context: &'a Context,
    ) -> BoxFuture<'a, Result<Self::Response, Error>>;
}

/// Object-safe runtime dispatch interface for dynamic dispatch routing of capabilities.
pub trait ErasedCapability<Target, Context, Error>: Send + Sync {
    /// Unique capability identifier name.
    fn name(&self) -> &'static str;
    /// Informative description of the capability.
    fn description(&self) -> &'static str;
    /// JSON Schema of the expected input payload.
    fn input_schema(&self) -> serde_json::Value;
    /// Erased invocation converting raw input arguments to strongly typed DTO calls.
    fn execute_erased<'a>(
        &'a self,
        target: &'a Target,
        payload: serde_json::Value,
        context: &'a Context,
    ) -> BoxFuture<'a, Result<serde_json::Value, Error>>;
}

impl<T, Target, Context, Error> ErasedCapability<Target, Context, Error> for T
where
    T: Capability<Target, Context, Error>,
    T::Request: serde::de::DeserializeOwned + Send + 'static,
    T::Response: serde::Serialize + Send + 'static,
    Target: 'static + Sync,
    Context: 'static + Sync,
    Error: 'static + From<serde_json::Error>,
{
    fn name(&self) -> &'static str {
        self.name()
    }

    fn description(&self) -> &'static str {
        self.description()
    }

    fn input_schema(&self) -> serde_json::Value {
        self.input_schema()
    }

    fn execute_erased<'a>(
        &'a self,
        target: &'a Target,
        payload: serde_json::Value,
        context: &'a Context,
    ) -> BoxFuture<'a, Result<serde_json::Value, Error>> {
        Box::pin(async move {
            let req: T::Request = serde_json::from_value(payload)?;
            let res = self.execute(target, req, context).await?;
            let serialized = serde_json::to_value(&res)?;
            Ok(serialized)
        })
    }
}

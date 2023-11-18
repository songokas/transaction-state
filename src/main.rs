use std::{any::Any, future::Future, marker::PhantomData, pin::Pin, process::Output};

struct AmlState {}

// struct OperationDefinition<
//     FactoryData,
//     FactoryResult,
//     OperationResult,
//     Operation: FnOnce(FactoryResult) -> F,
//     F: Future<Output = OperationResult>,
//     Factory: FnOnce(FactoryData) -> FactoryResult,
// > {
//     factory: Factory,
//     operation: Box<Operation>,
// }

// trait RunFactory {
//     fn run(&self);
// }

// trait RunOperation {
//     fn run(&self);
// }

// struct SagaOperation {}

// struct SagaFactory<Factory, FactoryResult, FactoryData>
// where
//     Factory: FnOnce(FactoryData) -> FactoryResult,
// {
//     callback: Factory,
//     _data: PhantomData<FactoryData>,
//     _result: PhantomData<FactoryResult>,
// }

// impl<FactoryResult, FactoryData, Factory> RunFactory
//     for SagaFactory<Factory, FactoryResult, FactoryData>
// where
//     Factory: FnOnce(FactoryData) -> FactoryResult,
// {
//     fn run(&self, data: FactoryData) -> FactoryResult {
//         (self.callback)(data)
//     }
// }

pub type FnAnyToAny = dyn FnOnce(Box<dyn Any>) -> Box<dyn Any>;

pub fn make_any_to_any<I, O, F>(f: F) -> Box<FnAnyToAny>
where
    I: 'static,
    O: 'static,
    F: FnOnce(I) -> O + 'static,
{
    Box::new(move |i: Box<dyn Any>| -> Box<dyn Any> {
        let i: Box<I> = Box::<dyn Any + 'static>::downcast(i).expect("wrong input type");
        Box::new(f(*i))
    })
}

fn combine<I: 'static, X, O: 'static, F1: 'static, F2: 'static>(
    f1: F1,
    f2: F2,
) -> impl FnOnce(I) -> Pin<Box<dyn Future<Output = O>>>
where
    F1: FnOnce(I) -> X,
    F2: FnOnce(X) -> Pin<Box<dyn Future<Output = O>>>,
{
    move |i| Box::pin(f2(f1(i)))
}

fn combine_f<I: 'static, X, O, F1: 'static, F2: 'static>(
    f1: F1,
    f2: F2,
) -> impl FnOnce(I) -> Pin<Box<dyn Future<Output = O>>>
where
    F1: FnOnce(I) -> Pin<Box<dyn Future<Output = X>>>,
    F2: FnOnce(X) -> Pin<Box<dyn Future<Output = O>>>,
{
    move |i| Box::pin(async { f2(f1(i).await).await })
}

struct SagaDefinition1<In, OperationResult> {
    name: &'static str,
    operation: Box<dyn FnOnce(In) -> Pin<Box<dyn Future<Output = OperationResult>>> + 'static>,
    // operation: Box<dyn FnOnce(FactoryResult) -> OperationResult>,
}

impl<FactoryData: 'static, OperationResult: 'static> SagaDefinition1<FactoryData, OperationResult> {
    fn new<Operation, Factory, FactoryResult: 'static>(
        name: &'static str,
        operation: Operation,
        factory: Factory,
    ) -> Self
    where
        Factory: FnOnce(FactoryData) -> FactoryResult + 'static,
        Operation:
            FnOnce(FactoryResult) -> Pin<Box<dyn Future<Output = OperationResult>>> + 'static,
    {
        Self {
            name,
            operation: Box::new(combine(factory, operation)),
            // factory: Box::new(factory),
        }
    }

    fn step<NewResult: 'static, Factory, Operation, NewFuture: 'static>(
        self,
        operation: Operation,
        factory: Factory,
    ) -> SagaDefinition1<FactoryData, NewFuture>
    where
        Operation: FnOnce(NewResult) -> Pin<Box<dyn Future<Output = NewFuture>>> + 'static,
        Factory: FnOnce(OperationResult) -> NewResult + 'static,
    {
        SagaDefinition1 {
            name: self.name,
            // combine(factory, operation),
            operation: Box::new(combine_f(self.operation, combine(factory, operation))),
        }
    }

    async fn run(self, data: FactoryData) -> OperationResult {
        (self.operation)(data).await
    }
}

struct SagaDefinition {
    name: &'static str,
    operations: Vec<(Box<FnAnyToAny>, Box<FnAnyToAny>)>,
}

impl SagaDefinition {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            operations: Vec::new(),
        }
    }
    fn step<
        Operation,
        Factory,
        FactoryData: 'static,
        FactoryResult: 'static,
        OperationResult,
        OperationFuture,
    >(
        mut self,
        operation: Operation,
        factory: Factory,
    ) -> Self
    where
        Factory: FnOnce(FactoryData) -> FactoryResult + 'static,
        Operation: FnOnce(FactoryResult) -> OperationFuture + 'static,
        OperationFuture: Future<Output = OperationResult> + 'static,
    {
        self.operations
            .push((make_any_to_any(operation), make_any_to_any(factory)));
        self
    }

    async fn run<In, Out>(self, data: In) -> Out
    where
        In: 'static,
        Out: 'static,
    {
        let input: Box<dyn Any + 'static> = Box::new(data);
        let output = self
            .operations
            .into_iter()
            .fold(input, |acc, (operation, factory)| {
                // let factory_callback: Box<dyn FnOnce(dyn Any) -> dyn Any> =
                //     factory.downcast().expect("factory");
                // // let operation_callback = operation.downcast().expect("operation");
                // // Box::new(operation_callback(factory_callback(data)))
                // Box::new((factory_callback)(*acc))
                factory(acc)
            });
        let o: Box<Out> = Box::<dyn Any + 'static>::downcast(output).expect("wrong output type");
        *o
    }
}

struct Transaction {}

async fn update_external_data(external_input: String) -> bool {
    println!("update_external_data with {external_input}");
    true
}

async fn db_transaction(callback: impl FnOnce(Transaction)) {
    callback(Transaction {});
    println!("db_transaction");
}

fn handle_external_data(external_data: bool) -> bool {
    println!("handle_external_data with {external_data}");
    external_data
}

async fn ts() -> bool {
    true
}

fn create_retry_definition() -> SagaDefinition1<String, bool> {
    SagaDefinition1::new(
        "create_retry_definition",
        |a| Box::pin(async { update_external_data(a).await }),
        |data| "a".to_string(),
    )
    .step(
        |a| Box::pin(async { update_external_data(a).await }),
        |data| data.to_string(),
    )
}

fn test1(a: bool) -> String {
    println!("1");
    "hello".to_string()
}

fn test2(b: String) -> i64 {
    println!("2");
    1
}

fn test3(b: i64) -> i32 {
    println!("3");
    1
}

// fn create_retry_definition1() -> SagaDefinition1<bool, i32> {
//     SagaDefinition1::new("create_retry_definition", test1)
//         .step(test2)
//         .step(test3)
// }

fn create_multiple_steps() -> SagaDefinition {
    SagaDefinition::new("create_multiple_steps")
        .step(update_external_data, |data| data)
        .step(
            |external_data| async {
                // must complete
                db_transaction(|_| {
                    // update record
                    // save external data
                })
                .await;
            },
            handle_external_data,
        )
}

fn create_multiple_steps_persist() -> SagaDefinition {
    SagaDefinition::new("create_multiple_steps_persist")
        .step(update_external_data, |initial_data| initial_data)
        .step(
            |external_data| async {
                // must complete
                db_transaction(|_| {
                    // update record
                    // save external data
                })
                .await;
            },
            handle_external_data,
        )
}

#[tokio::main]
async fn main() {
    // operation will run as long as transaction completes even if the server crashes
    let definition = create_retry_definition();
    let initial_data = "as".to_string();
    // must complete
    db_transaction(|transaction| {
        // update record
        // save operation event
        // definition.build(initial_data);
    })
    .await;
    let r: bool = definition.run(initial_data).await;

    // // operations will run one after another, will not rerun in case of a crash
    // let definition = create_multiple_steps();
    // let r: bool = definition.run(initial_data).await;

    // // operations will run one after another, will rerun in case of a crash
    // let definition = create_multiple_steps_persist();
    // let r: bool = definition.run(initial_data).await;
}

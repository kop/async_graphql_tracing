use actix_web::{guard, web, App, HttpResponse, HttpServer};
use anyhow::Result;
use async_graphql::{
    extensions::Tracing,
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptyMutation, EmptySubscription, Object as GQLObject, Schema,
};
use async_graphql_actix_web::{Request as GQLRequest, Response as GQLResponse};
use tracing::{info, info_span};
use tracing_futures::Instrument;

#[actix_rt::main]
async fn main() -> Result<()> {
    setup_logging_and_tracing()?;

    // Prepare schema
    let schema: MySchema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .extension(Tracing::default)
        .finish();

    // Start server
    HttpServer::new(move || {
        App::new()
            .data(schema.clone())
            .service(web::resource("/").guard(guard::Post()).to(index))
            .service(web::resource("/").guard(guard::Get()).to(index_playground))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}

// Initialize logging and tracing.
fn setup_logging_and_tracing() -> Result<()> {
    use opentelemetry::{
        api::{Key, Provider},
        sdk,
    };
    use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    // Filtering layer
    let layer_filter = match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(_) => EnvFilter::from_default_env(),
        Err(_) => EnvFilter::new("debug"),
    };

    // Logging layer
    let layer_logger = tracing_subscriber::fmt::layer();

    // OpenTelemetry layer
    let open_telemetry_exporter = opentelemetry_jaeger::Exporter::builder()
        .with_process(opentelemetry_jaeger::Process {
            service_name: env!("CARGO_PKG_NAME").to_string(),
            tags: vec![Key::new("version").string(env!("CARGO_PKG_VERSION").to_string())],
        })
        .init()?;
    let open_telemetry_provider = sdk::Provider::builder()
        .with_simple_exporter(open_telemetry_exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::AlwaysOn),
            ..Default::default()
        })
        .build();
    opentelemetry::global::set_provider(open_telemetry_provider.clone());
    let layer_open_telemetry = tracing_opentelemetry::layer()
        .with_tracer(open_telemetry_provider.get_tracer(env!("CARGO_PKG_NAME")));

    // Initialize
    tracing_subscriber::registry()
        .with(layer_filter)
        .with(layer_logger)
        .with(layer_open_telemetry)
        .try_init()?;

    Ok(())
}

/// Handler for "POST /".
async fn index(schema: web::Data<MySchema>, gql_req: GQLRequest) -> GQLResponse {
    let span: tracing::Span =
        info_span!("http_handler", request_id = %uuid::Uuid::new_v4().to_string());
    info!("hello from HTTP handler");
    GQLResponse::from(schema.execute(gql_req.into_inner()).instrument(span).await)
}

/// Handler for "GET /".
async fn index_playground() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(
            GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
        ))
}

type MySchema = Schema<Query, EmptyMutation, EmptySubscription>;

/// Root query object.
#[derive(Copy, Clone, Debug)]
pub struct Query;

#[GQLObject]
impl Query {
    async fn hello(&self) -> &str {
        "World!"
    }
}

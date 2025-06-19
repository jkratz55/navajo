use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use std::future::ready;
use std::time::Instant;
use futures::future::BoxFuture;
use sqlx::PgPool;

pub fn metrics_app() -> Router {
    const EXPONENTIAL_SECONDS: &[f64] = &[0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000];

    let prom_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full("db_query_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS   
        )
        .unwrap()
        .install_recorder()
        .unwrap();

    Router::new().route("/metrics", get(move || ready(prom_handle.render())))
}

pub async fn record_metrics(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}

pub async fn execute_query<T, E>(
    operation_name: &'static str,
    fut: BoxFuture<'static, Result<T, E>>,
) -> BoxFuture<'static, Result<T, E>>
where
    T: 'static,
    E: 'static 
{
    Box::pin(async move {
        let start = Instant::now();
        let result = fut.await;
        let duration = start.elapsed();

        let labels = [
            ("operation", operation_name),
            (
                "status",
                if result.is_ok() { "success" } else { "failure" }
            ),
        ];
        metrics::histogram!("db_query_duration_seconds", &labels)
            .record(duration.as_secs_f64());

        result
    })
}
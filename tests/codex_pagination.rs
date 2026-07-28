//! End-to-end check of the pagination chain: HTTP query string →
//! `CqrsHttpQuery` → `Pagination` → SQL → `Paged`.
//!
//! Needs both `rest` (for the extractor) and `surrealdb` (for a real storage
//! backend), so it only runs under `--all-features`.
#![cfg(all(feature = "rest", feature = "surrealdb"))]

use axum::extract::FromRequestParts;
use cqrs_rust_lib::read::query::Query;
use cqrs_rust_lib::read::storage::{HasId, Storage};
use cqrs_rust_lib::read::surrealdb::SurrealDBStorage;
use cqrs_rust_lib::read::Paged;
use cqrs_rust_lib::rest::CqrsHttpQuery;
use cqrs_rust_lib::read::{SortDirection, Sorter};
use cqrs_rust_lib::CqrsContext;
use serde::{Deserialize, Serialize};
use surrealdb::engine::any::connect;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Article {
    id: String,
    score: i32,
}

impl HasId for Article {
    fn field_id() -> &'static str {
        "id"
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn parent_field_id() -> Option<&'static str> {
        None
    }
    fn parent_id(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ArticleQuery {
    min_score: Option<i32>,
}

impl Query for ArticleQuery {
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![Sorter {
            field: "score".into(),
            direction: SortDirection::Asc,
        }])
    }
}

async fn store() -> SurrealDBStorage<Article, CqrsHttpQuery<ArticleQuery>> {
    let db = connect("mem://").await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    db.query("DEFINE TABLE IF NOT EXISTS articles SCHEMALESS")
        .await
        .unwrap()
        .check()
        .unwrap();
    let store = SurrealDBStorage::new(db, "article", "articles");
    for i in 1..=6i32 {
        store
            .save(
                Article {
                    id: format!("a{i}"),
                    score: i * 10,
                },
                CqrsContext::default(),
            )
            .await
            .unwrap();
    }
    store
}

async fn query(query_string: &str) -> CqrsHttpQuery<ArticleQuery> {
    let request = http::Request::builder()
        .uri(format!("/articles?{query_string}"))
        .body(())
        .unwrap();
    let (mut parts, _) = request.into_parts();
    CqrsHttpQuery::<ArticleQuery>::from_request_parts(&mut parts, &())
        .await
        .unwrap()
}

async fn page(query_string: &str) -> Paged<Article> {
    store()
        .await
        .filter(None, query(query_string).await, CqrsContext::default())
        .await
        .unwrap()
}

fn scores(page: &Paged<Article>) -> Vec<i32> {
    page.items.iter().map(|a| a.score).collect()
}

#[tokio::test]
async fn skip_and_limit_reach_the_storage() {
    let page = page("skip=3&limit=2").await;
    assert_eq!(page.total, 6);
    assert_eq!(page.skip, 3);
    assert_eq!(page.limit, 2);
    assert_eq!(scores(&page), vec![40, 50]);
}

#[tokio::test]
async fn page_and_page_size_still_work() {
    let page = page("page=1&page_size=2").await;
    assert_eq!(page.skip, 2);
    assert_eq!(page.limit, 2);
    assert_eq!(page.page, 1);
    assert_eq!(scores(&page), vec![30, 40]);
}

#[tokio::test]
async fn camel_case_page_size_alias_works() {
    let page = page("page=2&pageSize=2").await;
    assert_eq!(page.skip, 4);
    assert_eq!(scores(&page), vec![50, 60]);
}

#[tokio::test]
async fn skip_and_limit_win_over_page_params() {
    let page = page("skip=1&limit=1&page=2&pageSize=3").await;
    assert_eq!(page.skip, 1);
    assert_eq!(page.limit, 1);
    assert_eq!(scores(&page), vec![20]);
}

#[tokio::test]
async fn rsql_filter_combines_with_pagination() {
    let page = page("_q=score%3E%3D30&skip=1&limit=2").await;
    assert_eq!(page.total, 4, "score >= 30 matches 30,40,50,60");
    assert_eq!(scores(&page), vec![40, 50]);
}

#[tokio::test]
async fn defaults_apply_when_no_pagination_param_is_given() {
    let page = page("").await;
    assert_eq!(page.skip, 0);
    assert_eq!(page.limit, 20);
    assert_eq!(page.items.len(), 6);
}

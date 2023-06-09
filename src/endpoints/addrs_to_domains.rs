use crate::{models::AppState, utils::get_error};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use futures::stream::StreamExt;
use mongodb::{bson::doc, options::AggregateOptions};
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Serialize)]
pub struct AddrToDomainData {
    domain: Option<String>,
    address: FieldElement,
}

#[derive(Deserialize)]
pub struct AddrToDomainsQuery {
    addresses: Vec<FieldElement>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(query): Json<AddrToDomainsQuery>,
) -> impl IntoResponse {
    let domains = state.db.collection::<mongodb::bson::Document>("domains");

    let addresses: Vec<String> = query
        .addresses
        .iter()
        .map(|addr| addr.to_string())
        .collect();

    let pipeline = vec![
        doc! {
            "$match": {
                "addr": { "$in": addresses.clone() },
                "_chain.valid_to": null,
                "$expr": { "$eq": ["$addr", "$rev_addr"] },
            },
        },
        doc! {
            "$project": {
                "_id": 0,
                "domain": 1,
                "address": "$addr",
            },
        },
    ];

    let aggregate_options = AggregateOptions::default();
    let cursor = domains.aggregate(pipeline, aggregate_options).await;

    match cursor {
        Ok(mut cursor) => {
            let mut results = Vec::new();
            while let Some(doc) = cursor.next().await {
                if let Ok(doc) = doc {
                    let domain = doc.get_str("domain").map(|s| s.to_string()).ok();
                    let address = doc
                        .get_str("address")
                        .map(|s| FieldElement::from_str(s).unwrap())
                        .unwrap();
                    let data = AddrToDomainData { domain, address };
                    results.push(data);
                }
            }

            for address in &addresses {
                if !results
                    .iter()
                    .any(|data| data.address.to_string() == *address)
                {
                    results.push(AddrToDomainData {
                        domain: None,
                        address: FieldElement::from_str(address).unwrap(),
                    });
                }
            }

            (StatusCode::OK, Json(results)).into_response()
        }
        Err(_) => get_error("Error while fetching from database".to_string()),
    }
}

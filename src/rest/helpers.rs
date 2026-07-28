use crate::rest::ERROR_MEDIA_TYPE;
use crate::CqrsError;
use http::StatusCode;
use serde_json::Value;
use utoipa::openapi::path::{OperationBuilder, Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{
    Content, HttpMethod, PathItem, Paths, PathsBuilder, Ref, RefOr, Required, ResponseBuilder,
    Schema,
};
use utoipa::PartialSchema;

/// Component name of the error body in the OpenAPI document. The shape behind
/// it follows the `problem-json` feature (RFC 9457 document or legacy body).
pub const ERROR_SCHEMA_NAME: &str = "CqrsError";

/// `(name, schema)` pair to push into a route's schema list so the error body
/// ends up in `components/schemas`.
pub fn error_schema() -> (String, RefOr<Schema>) {
    (ERROR_SCHEMA_NAME.to_string(), CqrsError::schema())
}

pub fn method_to_string(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Delete => "delete",
        HttpMethod::Patch => "patch",
        HttpMethod::Options => "options",
        HttpMethod::Head => "head",
        HttpMethod::Trace => "trace",
    }
}

// One positional argument per OpenAPI facet; grouping them into a struct would
// only move the same list behind a name.
#[allow(clippy::too_many_arguments)]
pub fn generate_route(
    type_: &str,
    method: HttpMethod,
    path: &str,
    response: RefOr<Schema>,
    path_parameters: Vec<(String, RefOr<Schema>)>,
    query_parameters: Vec<Parameter>,
    body: Option<RefOr<Schema>>,
    error_statuses: &[StatusCode],
) -> Paths {
    let code = match &method {
        HttpMethod::Post => "201",
        _ => "200",
    };
    let mut operation = OperationBuilder::new()
        .response(
            code,
            ResponseBuilder::new().content("application/json", Content::new(Some(response))),
        )
        .operation_id(Some(format!(
            "{}-{}-{}",
            type_,
            method_to_string(&method),
            path.replace("/", "-")
        )))
        .parameters(if query_parameters.is_empty() {
            None
        } else {
            Some(query_parameters)
        })
        .tag(type_);

    for status in error_statuses {
        operation = operation.response(
            status.as_u16().to_string(),
            ResponseBuilder::new()
                .description(status.canonical_reason().unwrap_or("Error"))
                .content(
                    ERROR_MEDIA_TYPE,
                    Content::new(Some(RefOr::Ref(Ref::from_schema_name(ERROR_SCHEMA_NAME)))),
                ),
        );
    }

    for (name, schema) in path_parameters {
        operation = operation.parameter(
            ParameterBuilder::new()
                .name(name)
                .parameter_in(ParameterIn::Path)
                .required(Required::True)
                .schema(Some(schema)),
        );
    }

    if let Some(body) = body {
        operation = operation.request_body(Some(
            RequestBody::builder()
                .content("application/json", Content::new(Some(body)))
                .build(),
        ));
    }
    PathsBuilder::new()
        .path(path, PathItem::new(method, operation.build()))
        .build()
}

pub struct SchemaData {
    pub name: String,
    pub schema: Schema,
    pub discriminator: Option<(String, String)>,
}

impl SchemaData {
    fn new(name: String, schema: Schema, discriminator: Option<(String, String)>) -> Self {
        Self {
            name,
            schema,
            discriminator,
        }
    }
}

pub fn read_schema(name: &str, schema: RefOr<Schema>) -> Vec<SchemaData> {
    let mut result = vec![];
    if let RefOr::T(t) = &schema {
        match t {
            Schema::Object(o) => {
                let discriminator = o.properties.iter().find_map(|property| match property.1 {
                    // Opinionated: only one discriminator is allowed per schema
                    RefOr::T(Schema::Object(o)) => o
                        .enum_values
                        .clone()
                        .filter(|e| e.len() == 1)
                        .and_then(|e| match &e[0] {
                            Value::String(s) => Some((property.0.to_string(), s.to_string())),
                            _ => None,
                        }),
                    _ => None,
                });
                let (current_name, schema_body) = if let Some((f, value)) = &discriminator {
                    match t {
                        Schema::Object(o) => {
                            let mut body = o.clone();
                            body.properties.remove(f);
                            (value.to_string(), Schema::Object(body))
                        }
                        _ => (name.to_string(), t.clone()),
                    }
                } else {
                    (name.to_string(), t.clone())
                };
                result.push(SchemaData::new(current_name, schema_body, discriminator));
            }
            Schema::OneOf(items) => {
                for item in &items.items {
                    result.extend(read_schema(name, item.clone()));
                }
            }
            Schema::AnyOf(items) => {
                for item in &items.items {
                    result.extend(read_schema(name, item.clone()));
                }
            }
            _ => (),
        }
    }
    result
}
#[allow(clippy::collapsible_if)]
pub fn add_discriminator(item: &mut Value, discriminator: Option<(String, String)>) {
    if let Some((name, value)) = discriminator {
        if let Some(obj) = item.as_object_mut() {
            obj.insert(name, value.into());
        }
    }
}

fn strip_suffixes<'a>(s: &'a str, suffixes: &[&str]) -> &'a str {
    let mut result = s;
    for suffix in suffixes {
        if result.ends_with(suffix) {
            result = &result[..result.len() - suffix.len()]
        }
    }
    result
}
pub fn sanitize_schema_name(name: &str) -> String {
    let to_remove = ["Command", "Commands", "Query", "Queries"];
    let mut result = String::new();
    let mut prev_char: Option<char> = None;
    let name_to_process = strip_suffixes(name, &to_remove);

    for (i, c) in name_to_process.chars().enumerate() {
        if i == 0 {
            result.push(c.to_ascii_lowercase());
            prev_char = Some(c);
            continue;
        }

        if c.is_uppercase() && prev_char.is_some_and(|pc| pc.is_lowercase()) {
            result.push('-');
        }

        result.push(c.to_ascii_lowercase());
        prev_char = Some(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_error_responses_with_the_error_media_type() {
        let paths = generate_route(
            "game",
            HttpMethod::Get,
            "/games/{game_id}",
            RefOr::Ref(Ref::from_schema_name("Game")),
            vec![("game_id".to_string(), String::schema())],
            vec![],
            None,
            &[StatusCode::NOT_FOUND, StatusCode::INTERNAL_SERVER_ERROR],
        );

        let json = serde_json::to_value(&paths).unwrap();
        let responses = &json["/games/{game_id}"]["get"]["responses"];

        assert!(responses["200"]["content"]["application/json"].is_object());

        for status in ["404", "500"] {
            let content = &responses[status]["content"][ERROR_MEDIA_TYPE];
            assert!(
                content.is_object(),
                "missing {ERROR_MEDIA_TYPE} content for {status}: {responses}"
            );
            assert_eq!(
                content["schema"]["$ref"],
                format!("#/components/schemas/{ERROR_SCHEMA_NAME}")
            );
        }
        assert_eq!(responses["404"]["description"], "Not Found");
    }

    #[test]
    fn without_error_statuses_only_the_success_response_is_declared() {
        let paths = generate_route(
            "game",
            HttpMethod::Get,
            "/games",
            RefOr::Ref(Ref::from_schema_name("Game")),
            vec![],
            vec![],
            None,
            &[],
        );
        let json = serde_json::to_value(&paths).unwrap();
        let responses = json["/games"]["get"]["responses"].as_object().unwrap();
        assert_eq!(responses.len(), 1);
        assert!(responses.contains_key("200"));
    }
}

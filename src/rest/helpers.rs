use serde_json::Value;
use utoipa::openapi::path::{OperationBuilder, Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{
    Content, HttpMethod, PathItem, Paths, PathsBuilder, RefOr, Required, ResponseBuilder, Schema,
};

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

pub fn generate_route(
    type_: &str,
    method: HttpMethod,
    path: &str,
    response: RefOr<Schema>,
    path_parameters: Vec<(String, RefOr<Schema>)>,
    query_parameters: Vec<Parameter>,
    body: Option<RefOr<Schema>>,
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

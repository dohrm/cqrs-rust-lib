use utoipa::openapi::path::{OperationBuilder, ParameterBuilder, ParameterIn};
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
    type_: &'static str,
    method: HttpMethod,
    path: &str,
    response: RefOr<Schema>,
    path_parameters: Vec<(&str, RefOr<Schema>)>,
    query_parameters: Vec<(&str, RefOr<Schema>, bool)>,
    body: Option<Schema>,
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
        // .deprecated(Some(Deprecated::False))
        // .summary(Some("Summary"))
        // .description(Some("Description"))
        .tag(type_);

    for (name, schema) in path_parameters {
        operation = operation.parameter(
            ParameterBuilder::new()
                .name(name)
                .parameter_in(ParameterIn::Path)
                .required(Required::True)
                // .deprecated(Some(Deprecated::False))
                // .description(Some("xxx"))
                .schema(Some(schema)),
        );
    }

    for (name, schema, required) in query_parameters {
        operation = operation.parameter(
            ParameterBuilder::new()
                .name(name)
                .parameter_in(ParameterIn::Query)
                .required(if required {
                    Required::True
                } else {
                    Required::False
                })
                // .deprecated(Some(Deprecated::False))
                // .description(Some("xxx"))
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

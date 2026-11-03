use crate::api::{Denied, Error};
use utoipa::openapi::path::{HttpMethod, OperationBuilder, ParameterBuilder, ParameterIn};
use utoipa::openapi::{
    ComponentsBuilder, ContentBuilder, InfoBuilder, OpenApi, OpenApiBuilder, PathItem,
    PathsBuilder, Ref, Required, ResponseBuilder, ResponsesBuilder,
};
use utoipa::PartialSchema;

pub const GATEWAY_PATH: &str = "/{key}/{host}/{path}";

fn parameter(
    name: &str,
    place: ParameterIn,
    required: bool,
    description: &str,
) -> ParameterBuilder {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(place)
        .required(if required {
            Required::True
        } else {
            Required::False
        })
        .description(Some(description))
        .schema(Some(String::schema()))
}

fn json(description: &str, schema: &str) -> ResponseBuilder {
    ResponseBuilder::new().description(description).content(
        "application/json",
        ContentBuilder::new()
            .schema(Some(Ref::from_schema_name(schema)))
            .build(),
    )
}

pub fn gateway() -> OpenApi {
    let operation = OperationBuilder::new()
        .operation_id(Some("forward"))
        .description(Some(
            "Sends the call on to https://<host>/<path>?<query> with the key's secret injected. Method, other headers and body pass as they are; the answer streams back with the secret replaced by [redacted]; redirects are not followed.",
        ))
        .parameter(parameter("key", ParameterIn::Path, true, "The vault key to inject"))
        .parameter(parameter("host", ParameterIn::Path, true, "Where the call goes, host[:port]"))
        .parameter(parameter("path", ParameterIn::Path, true, "The upstream path and query"))
        .parameter(parameter(
            "Authorization",
            ParameterIn::Header,
            true,
            "Bearer vt_…, a token made for this sandbox",
        ))
        .parameter(parameter(
            "X-Vault-Request",
            ParameterIn::Header,
            false,
            "Agent tokens only: what this call is for, percent-encoded UTF-8",
        ))
        .responses(
            ResponsesBuilder::new()
                .response(
                    "default",
                    ResponseBuilder::new().description("The upstream answer, passed through"),
                )
                .response("400", json("X-Vault-Request is not percent-encoded UTF-8", "Error"))
                .response(
                    "403",
                    json(
                        "Refused: token_unknown, token_expired, token_used_up, wrong_sandbox, key_not_allowed, method_not_allowed, url_not_allowed, vetting_denied, vetting_unavailable, approval_denied, approval_timeout or wrong_user",
                        "Denied",
                    ),
                )
                .response("500", json("The token's use could not be saved", "Error"))
                .response("502", json("The upstream could not be reached or answered in a way the vault cannot read", "Error")),
        );
    let methods = [
        HttpMethod::Get,
        HttpMethod::Head,
        HttpMethod::Options,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ];
    OpenApiBuilder::new()
        .info(
            InfoBuilder::new()
                .title("ai2 vault gateway")
                .version(env!("CARGO_PKG_VERSION"))
                .description(Some(
                    "A sandbox endpoint. Any HTTP client calls it as if it were the service: baseUrl = <endpoint>/<key>/<host>/<base path>, apiKey = vt_…",
                )),
        )
        .paths(
            PathsBuilder::new().path(GATEWAY_PATH, PathItem::from_http_methods(methods, operation)),
        )
        .components(Some(
            ComponentsBuilder::new()
                .schema_from::<Error>()
                .schema_from::<Denied>()
                .build(),
        ))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gateway_document_has_its_one_path_for_every_method() {
        let doc = serde_json::to_value(gateway()).unwrap();
        let item = &doc["paths"][GATEWAY_PATH];
        for method in ["get", "head", "options", "post", "put", "patch", "delete"] {
            assert_eq!(
                item[method]["responses"]["403"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/Denied",
                "{method}"
            );
        }
        let names: Vec<&str> = item["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["key", "host", "path", "Authorization", "X-Vault-Request"]
        );
        assert!(doc["components"]["schemas"]["Denied"]["properties"]["reason"].is_object());
    }
}

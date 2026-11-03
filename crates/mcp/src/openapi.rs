use rmcp::model::{JsonObject, Tool};
use serde_json::{json, Map, Value};

const DEFS: &str = "#/$defs/";
const COMPONENTS: &str = "#/components/schemas/";

fn rewrite(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix(DEFS) {
                    *reference = format!("{COMPONENTS}{name}");
                }
            }
            object.values_mut().for_each(rewrite);
        }
        Value::Array(items) => items.iter_mut().for_each(rewrite),
        _ => {}
    }
}

fn hoisted(schema: &JsonObject, components: &mut Map<String, Value>) -> Value {
    let mut schema = schema.clone();
    schema.remove("$schema");
    if let Some(Value::Object(defs)) = schema.remove("$defs") {
        for (name, mut def) in defs {
            rewrite(&mut def);
            components.entry(name).or_insert(def);
        }
    }
    let mut schema = Value::Object(schema);
    rewrite(&mut schema);
    schema
}

fn body(schema: Value) -> Value {
    json!({ "content": { "application/json": { "schema": schema } } })
}

pub fn document(tools: &[Tool], name: &str, version: &str, server: &str) -> Value {
    let mut components = Map::new();
    let mut paths = Map::new();
    for tool in tools {
        let input = hoisted(&tool.input_schema, &mut components);
        let mut ok = match &tool.output_schema {
            Some(output) => body(hoisted(output, &mut components)),
            None => json!({}),
        };
        ok["description"] = json!("The tool's structured result");
        let mut request = body(input);
        request["required"] = json!(true);
        let operation = json!({
            "operationId": tool.name,
            "summary": tool.description.as_deref().unwrap_or_default(),
            "x-mcp": { "tool": tool.name },
            "requestBody": request,
            "responses": {
                "200": ok,
                "default": {
                    "description": "The tool failed: { error, message }",
                    "content": { "application/json": { "schema": { "$ref": format!("{COMPONENTS}Failure") } } }
                }
            }
        });
        paths.insert(
            format!("/tools/{}", tool.name),
            json!({ "post": operation }),
        );
    }
    components.insert(
        "Failure".into(),
        json!({
            "type": "object",
            "properties": { "error": { "type": "string" }, "message": { "type": "string" } },
            "required": ["error", "message"]
        }),
    );
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": format!("{name} MCP tools"),
            "version": version,
            "description": format!("Every MCP tool {name} serves, as an operation: the request body is the tool's inputSchema and the 200 body its outputSchema. Call them over MCP at the server below, with tools/call.")
        },
        "servers": [ { "url": server } ],
        "paths": paths,
        "components": { "schemas": components }
    })
}

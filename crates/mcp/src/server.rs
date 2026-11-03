use crate::clients::{Caller, Clients, Seen};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{
    Implementation, InitializeRequestParams, InitializeResult, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{tool_handler, ErrorData, RoleServer, ServerHandler};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct Pieces {
    pub name: &'static str,
    pub version: &'static str,
    pub clients: Clients,
    pub reviews: Arc<dyn crate::reviews::Reviews>,
    #[cfg(feature = "harness")]
    pub harness: Arc<dyn crate::harness::Harnesses>,
    #[cfg(feature = "vault")]
    pub vault: Arc<dyn crate::vault::Admin>,
    #[cfg(feature = "setup")]
    pub setup: Arc<dyn crate::setup::Setup>,
    #[cfg(feature = "apps")]
    pub apps: Arc<dyn crate::apps::Apps>,
    #[cfg(feature = "agents")]
    pub agents: ::agents::Agents,
}

impl Pieces {
    pub fn local(name: &'static str, version: &'static str, bin: PathBuf, home: PathBuf) -> Self {
        let reviews: Arc<dyn crate::reviews::Reviews> =
            Arc::new(crate::reviews::Child::new(bin.clone(), home.clone()));
        Self {
            name,
            version,
            clients: Clients::at(&home),
            #[cfg(feature = "agents")]
            agents: ::agents::Agents::local(&home, ::harness::Registry::public)
                .with_review(Arc::new(crate::agents::Reviewing(reviews.clone()))),
            reviews,
            #[cfg(feature = "harness")]
            harness: Arc::new(crate::harness::local(
                ::harness::Registry::public,
                home.clone(),
            )),
            #[cfg(feature = "vault")]
            vault: Arc::new(crate::vault::Socket(::vault::default_paths(&home).1)),
            #[cfg(feature = "setup")]
            setup: Arc::new(crate::setup::Local(::setup::Options::new(
                bin,
                home.clone(),
            ))),
            #[cfg(feature = "apps")]
            apps: match crate::apps::Local::detached(&home) {
                Ok(local) => Arc::new(local),
                Err(failure) => Arc::new(crate::apps::Unavailable(failure)),
            },
        }
    }
}

#[derive(Clone)]
pub struct Server {
    pub(crate) pieces: Pieces,
    tools: ToolRouter<Server>,
    transport: &'static str,
}

pub fn tools() -> ToolRouter<Server> {
    [
        Server::reviews_tools(),
        #[cfg(feature = "harness")]
        Server::harness_tools(),
        #[cfg(feature = "vault")]
        Server::vault_tools(),
        #[cfg(feature = "setup")]
        Server::setup_tools(),
        #[cfg(feature = "apps")]
        Server::apps_tools(),
        #[cfg(feature = "agents")]
        Server::agents_tools(),
    ]
    .into_iter()
    .fold(ToolRouter::new(), |tools, piece| tools + piece)
}

fn instructions(name: &str) -> String {
    [
        format!("{name} as tools. Every tool answers with JSON. reviews_* rate a coding agent's run on a folder (the Modern Security Rating and the Run Risk Factor)."),
        #[cfg(feature = "harness")]
        format!("harness_* list, install and sign in to coding agents and print their headless command lines; a sign-in that needs a code typed in is done in the CLI or {name} ui."),
        #[cfg(feature = "vault")]
        format!("vault_* manage the vault, which keeps secrets away from agents: keys are sent only to their approved URLs through the gateway. No tool returns a secret; only vault_create_token returns the token it made, bound to its sandbox, and never its revoke value. A change asks the person at the machine (Touch ID); where it cannot, the tool answers presence_required and the change is approved in {name} ui or the CLI. Running a command with keys injected (vault with) is done by the CLI."),
        #[cfg(feature = "setup")]
        format!("setup_check reports whether this machine is set up for the vault; {name} setup changes it."),
        #[cfg(feature = "apps")]
        "apps_* register, list, start and stop the local apps; every app has an OpenAPI document, which apps_list returns with include_openapi.".to_string(),
        #[cfg(feature = "agents")]
        "agents_* keep agents (a harness, a model and a personality by name); agent_run runs one on a prompt in a folder and reviews the folder after; runs_* read the run records.".to_string(),
    ]
    .join(" ")
}

impl Server {
    pub fn new(pieces: Pieces) -> Self {
        Self::with_tools(pieces, tools())
    }

    pub fn with_tools(pieces: Pieces, tools: ToolRouter<Server>) -> Self {
        Self {
            pieces,
            tools,
            transport: "stdio",
        }
    }

    pub fn over(mut self, transport: &'static str) -> Self {
        self.transport = transport;
        self
    }

    fn seen(
        &self,
        request: &InitializeRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> Seen {
        let token = context
            .extensions
            .get::<http::request::Parts>()
            .and_then(|parts| parts.extensions.get::<Caller>())
            .map(|caller| caller.token.clone());
        Seen {
            name: request.client_info.name.clone(),
            version: request.client_info.version.clone(),
            transport: self.transport.to_string(),
            token,
        }
    }

    pub fn tool_list(&self) -> Vec<rmcp::model::Tool> {
        self.tools.list_all()
    }

    pub fn openapi(&self, server: &str) -> serde_json::Value {
        crate::openapi::document(
            &self.tool_list(),
            self.pieces.name,
            self.pieces.version,
            server,
        )
    }
}

#[tool_handler(router = self.tools)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(self.pieces.name, self.pieces.version))
            .with_instructions(instructions(self.pieces.name))
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        context.peer.set_peer_info(request.clone());
        let seen = self.seen(&request, &context);
        let clients = self.pieces.clients.clone();
        let _ =
            tokio::task::spawn_blocking(move || clients.record(seen, crate::clients::now())).await;
        self.negotiate_initialize(&request)
    }
}

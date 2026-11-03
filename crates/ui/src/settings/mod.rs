mod providers;
#[cfg(test)]
mod tests;

use crate::harnesses::Harnesses;
use crate::reply::error;
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use harness::settings::{Security, Settings as _, VaultUse, AUTO, MODEL, NONE};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub type Restart = Arc<dyn Fn() -> anyhow::Result<String> + Send + Sync>;

#[derive(Clone)]
pub struct Context {
    pub harnesses: Arc<Harnesses>,
    pub restart: Restart,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct Choice {
    pub runs: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Saved {
    pub page: String,
    pub choice: Choice,
    pub saved: bool,
    pub restart: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Tested {
    pub ok: bool,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Security,
    Vault,
}

impl Page {
    fn parse(page: &str) -> Option<Self> {
        match page {
            "security" => Some(Self::Security),
            "vault" => Some(Self::Vault),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Vault => "vault",
        }
    }

    fn load(self, home: &std::path::Path) -> anyhow::Result<Option<Choice>> {
        Ok(match self {
            Self::Security => Security::load(home)?.map(|s| Choice {
                runs: s.reviewer,
                provider: s.provider,
                model: s.model,
            }),
            Self::Vault => VaultUse::load(home)?.map(|s| Choice {
                runs: s.vetter,
                provider: s.provider,
                model: s.model,
            }),
        })
    }

    fn fallback(self) -> Choice {
        Choice {
            runs: match self {
                Self::Security => AUTO.into(),
                Self::Vault => MODEL.into(),
            },
            ..Choice::default()
        }
    }

    fn save(self, home: &std::path::Path, choice: &Choice) -> anyhow::Result<()> {
        match self {
            Self::Security => Security {
                reviewer: choice.runs.clone(),
                provider: choice.provider.clone(),
                model: choice.model.clone(),
            }
            .save(home),
            Self::Vault => VaultUse {
                vetter: choice.runs.clone(),
                provider: choice.provider.clone(),
                model: choice.model.clone(),
            }
            .save(home),
        }
    }

    fn fixed(self) -> &'static [&'static str] {
        match self {
            Self::Security => &[AUTO, NONE],
            Self::Vault => &[],
        }
    }
}

pub fn routes() -> OpenApiRouter<Shared> {
    with(Context {
        harnesses: Arc::new(Harnesses::public()),
        restart: Arc::new(restart_vault),
    })
}

pub fn with(context: Context) -> OpenApiRouter<Shared> {
    OpenApiRouter::new()
        .routes(routes!(get_settings, put_settings))
        .routes(routes!(test_settings))
        .routes(routes!(restart))
        .merge(providers::routes())
        .layer(Extension(context))
}

fn unknown_page() -> Response {
    error(404, "not_found", "There are no settings by that name.")
}

#[utoipa::path(
    get,
    path = "/settings/{page}",
    tag = "settings",
    params(("page" = String, Path, description = "security or vault")),
    responses(
        (status = 200, description = "What runs the page's checks; saved is false when nothing was chosen yet", body = Saved),
        (status = 404, description = "There is no such page", body = crate::Reply)
    )
)]
async fn get_settings(State(state): State<Shared>, Path(page): Path<String>) -> Response {
    let Some(page) = Page::parse(&page) else {
        return unknown_page();
    };
    match page.load(&state.home) {
        Ok(saved) => Json(Saved {
            page: page.name().into(),
            saved: saved.is_some(),
            choice: saved.unwrap_or_else(|| page.fallback()),
            restart: false,
        })
        .into_response(),
        Err(failure) => error(500, "internal", &format!("{failure:#}")),
    }
}

fn given(text: &Option<String>) -> Option<String> {
    text.as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn checked(
    page: Page,
    choice: Choice,
    context: &Context,
    home: &std::path::Path,
) -> Result<Choice, String> {
    let runs = choice.runs.trim().to_string();
    let model = given(&choice.model);
    if page.fixed().contains(&runs.as_str()) {
        return Ok(Choice {
            runs,
            provider: None,
            model: None,
        });
    }
    if runs == MODEL {
        let provider = given(&choice.provider).ok_or("Pick a model provider.")?;
        let known = providers::read(home)
            .map_err(|failure| format!("{failure:#}"))?
            .into_iter()
            .any(|known| known.name == provider);
        if !known {
            return Err(format!("There is no provider called {provider}."));
        }
        let model = model.ok_or("Pick a model.")?;
        return Ok(Choice {
            runs,
            provider: Some(provider),
            model: Some(model),
        });
    }
    if !context.harnesses.can_run(&runs) {
        return Err(format!(
            "There is no harness called {runs} that runs on its own."
        ));
    }
    Ok(Choice {
        runs,
        provider: None,
        model,
    })
}

#[utoipa::path(
    put,
    path = "/settings/{page}",
    tag = "settings",
    params(("page" = String, Path, description = "security or vault")),
    request_body = Choice,
    responses(
        (status = 200, description = "Saved; restart is true when the vault reads it only when it starts", body = Saved),
        (status = 400, description = "The choice is not one that can run", body = crate::Reply),
        (status = 404, description = "There is no such page", body = crate::Reply)
    )
)]
async fn put_settings(
    State(state): State<Shared>,
    Extension(context): Extension<Context>,
    Path(page): Path<String>,
    Json(choice): Json<Choice>,
) -> Response {
    let Some(page) = Page::parse(&page) else {
        return unknown_page();
    };
    let choice = match checked(page, choice, &context, &state.home) {
        Ok(choice) => choice,
        Err(message) => return error(400, "bad_request", &message),
    };
    if let Err(failure) = page.save(&state.home, &choice) {
        return error(500, "internal", &format!("{failure:#}"));
    }
    Json(Saved {
        page: page.name().into(),
        choice,
        saved: true,
        restart: page == Page::Vault,
    })
    .into_response()
}

#[utoipa::path(
    post,
    path = "/settings/{page}/test",
    tag = "settings",
    params(("page" = String, Path, description = "security or vault")),
    request_body = Choice,
    responses(
        (status = 200, description = "Whether the choice is ready: a harness is set up, or the model answers through the vault", body = Tested),
        (status = 404, description = "There is no such page", body = crate::Reply)
    )
)]
async fn test_settings(
    State(state): State<Shared>,
    Extension(context): Extension<Context>,
    Path(page): Path<String>,
    Json(choice): Json<Choice>,
) -> Response {
    let Some(page) = Page::parse(&page) else {
        return unknown_page();
    };
    let choice = match checked(page, choice, &context, &state.home) {
        Ok(choice) => choice,
        Err(message) => return Json(Tested { ok: false, message }).into_response(),
    };
    let tested = match choice.runs.as_str() {
        AUTO => Tested {
            ok: true,
            message: "The first harness that is set up runs.".into(),
        },
        NONE => Tested {
            ok: true,
            message: "Nothing runs, so this check is left out.".into(),
        },
        MODEL => {
            let provider = choice.provider.unwrap_or_default();
            let model = choice.model.unwrap_or_default();
            providers::test(&state, &provider, Some(&model)).await
        }
        id => {
            let ready = context.harnesses.ready(id);
            Tested {
                ok: ready,
                message: if ready {
                    "Set up and signed in.".into()
                } else {
                    "Not set up and signed in yet.".into()
                },
            }
        }
    };
    Json(tested).into_response()
}

#[utoipa::path(
    post,
    path = "/settings/vault/restart",
    tag = "settings",
    responses(
        (status = 200, description = "The vault started again and reads its settings", body = Tested),
        (status = 502, description = "The vault could not be started again", body = crate::Reply)
    )
)]
async fn restart(Extension(context): Extension<Context>) -> Response {
    let restart = context.restart.clone();
    match tokio::task::spawn_blocking(move || restart()).await {
        Ok(Ok(message)) => Json(Tested { ok: true, message }).into_response(),
        Ok(Err(failure)) => error(502, "restart_failed", &format!("{failure:#}")),
        Err(failure) => error(500, "internal", &failure.to_string()),
    }
}

pub fn restart_line(macos: bool, uid: u32, label: &str) -> setup::runner::Cmd {
    use setup::runner::Cmd;
    if macos {
        Cmd::new(
            "launchctl",
            &["kickstart", "-k", &format!("gui/{uid}/{label}")],
        )
    } else {
        Cmd::new("systemctl", &["restart", setup::files::UNIT]).root_if(false)
    }
}

fn restart_vault() -> anyhow::Result<String> {
    use setup::runner::{Runner, System};
    let host = setup::host::Host::detect(&System);
    let macos = matches!(host.os, setup::host::Os::Macos { .. });
    let label = setup::files::label(std::env::var(setup::files::LABEL_ENV).ok());
    let line = restart_line(macos, host.uid, &label);
    if System.run(&line)?.ok {
        Ok("The vault started again.".into())
    } else {
        anyhow::bail!("{} did not work.", line.line())
    }
}

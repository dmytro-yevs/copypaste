//! AI Tools: an MCP server that approved AI assistants reach through the
//! module's `mcp` endpoint (`copypaste module connect copypaste.ai-tools
//! mcp`). The exposure policy and grants sync as module data, approvals
//! are host prompts, and the host hides sensitive history before the module
//! sees it.
#![deny(unsafe_code)]

mod access_log;
mod calls;
mod endpoint;
mod grants;
mod host;
mod limits;
mod policy;
mod service;
mod spec;
mod strings;
mod tools;
mod views;

#[cfg(test)]
mod tests;

use copypaste_module_sdk::native::HostClient;
use copypaste_module_sdk::{
    HostError, HostErrorCode, Module, ModuleDelivery, ModuleEnvironment, ModuleInvocation,
    ModuleOutput,
};
use service::Service;
use std::sync::Arc;

const MSG_NEEDS_HOST: &str = "This version of CopyPaste cannot run AI tools yet.";

pub struct AiTools {
    /// `None` when the host offers no services; only `setup` works then.
    service: Option<Arc<Service>>,
    endpoints: endpoint::Endpoints,
}

impl AiTools {
    fn with_host(host: Arc<dyn host::Host>) -> Self {
        Self {
            service: Some(Arc::new(Service::new(host))),
            endpoints: endpoint::Endpoints::default(),
        }
    }
}

impl Module for AiTools {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Ok(Self {
            service: None,
            endpoints: endpoint::Endpoints::default(),
        })
    }

    fn create_with_host(_: ModuleEnvironment, host: HostClient) -> Result<Self, String> {
        Ok(Self::with_host(Arc::new(host)))
    }

    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        run(self.service.as_deref(), &invocation).map(|view| ModuleOutput::View { view })
    }

    fn deliver(&mut self, delivery: ModuleDelivery) -> Result<(), String> {
        let service = self.service.as_ref().ok_or(MSG_NEEDS_HOST)?;
        self.endpoints.deliver(service, delivery)
    }
}

impl Drop for AiTools {
    fn drop(&mut self) {
        if let Some(service) = &self.service {
            self.endpoints.shutdown(service);
            service.flush_log();
        }
    }
}

fn message(error: HostError) -> String {
    match error.code {
        HostErrorCode::Unsupported => MSG_NEEDS_HOST.to_owned(),
        _ => error.message,
    }
}

fn run(
    service: Option<&Service>,
    invocation: &ModuleInvocation,
) -> Result<copypaste_module_sdk::ModuleView, String> {
    if let Some(service) = service {
        service.remember_preferences(&invocation.preferences);
    }
    let locale = service.map_or_else(|| "en".to_owned(), Service::locale);
    if invocation.command == "setup" {
        return Ok(views::setup(&locale));
    }
    let service = service.ok_or(MSG_NEEDS_HOST)?;
    let grant = || {
        invocation
            .arguments
            .get("grant")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| "That AI tool connection no longer exists.".to_owned())
    };
    let view = match invocation.command.as_str() {
        "connections" => views::connections(service, &locale),
        "activity" => views::activity(service, &locale, grant()?),
        "set-access" => {
            let write = invocation
                .arguments
                .get("write")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            service
                .set_access(grant()?, write)
                .and_then(|()| views::connections(service, &locale))
        }
        "revoke" => service
            .revoke(grant()?)
            .and_then(|()| views::connections(service, &locale)),
        "remove" => service
            .remove(grant()?)
            .and_then(|()| views::connections(service, &locale)),
        _ => return Err("This command is not available.".into()),
    }
    .map_err(message)?;
    view.validate()?;
    Ok(view)
}

copypaste_module_sdk::export_host_module!(AiTools);

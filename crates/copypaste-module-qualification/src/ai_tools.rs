//! The AI tools package: its declared host services and endpoint, the
//! connection instructions it renders without any host service, and the
//! enable gate.
use copypaste_module_sdk::{ModuleCapability, ModulePlatform, ModuleView, ViewItem};
use copypaste_modules::{InstalledModule, ModuleManager, ModuleOutput, ModuleTarget};
use std::collections::BTreeMap;

const CONNECT: &str = "module connect copypaste.ai-tools mcp";

pub(super) fn qualify(
    manager: &ModuleManager,
    installed: &InstalledModule,
    target: ModuleTarget,
) -> Result<usize, String> {
    let id = &installed.id;
    if installed.capabilities.len() != 8
        || !installed
            .capabilities
            .contains(&ModuleCapability::Endpoints)
    {
        return Err("The AI tools package must declare its host services.".into());
    }
    let endpoint = installed
        .endpoints
        .iter()
        .find(|endpoint| endpoint.id == "mcp")
        .ok_or("The AI tools package must declare its MCP endpoint.")?;
    if endpoint.platforms.contains(&ModulePlatform::Android) {
        return Err("Android must not serve the MCP endpoint.".into());
    }
    manager.set_enabled(id, false).map_err(|e| e.to_string())?;
    if manager.invoke(id, "setup", BTreeMap::new()).is_ok() {
        return Err("A disabled AI tools module accepted commands.".into());
    }
    manager.set_enabled(id, true).map_err(|e| e.to_string())?;
    let output = manager
        .invoke(id, "setup", BTreeMap::new())
        .map_err(|e| e.to_string())?;
    let ModuleOutput::View { view } = output else {
        return Err("The AI tools package did not return its instructions.".into());
    };
    view.validate()?;
    let shows_command = blocks(&view).any(|text| text.contains(CONNECT));
    if target.platform == ModulePlatform::Android {
        if shows_command {
            return Err("Android must point to a computer, not an MCP command.".into());
        }
    } else if !shows_command {
        return Err("The instructions must show the connect command.".into());
    }
    Ok(3)
}

fn blocks(view: &ModuleView) -> impl Iterator<Item = &str> {
    view.sections
        .iter()
        .flat_map(|section| &section.items)
        .filter_map(|item| match item {
            ViewItem::Block { text, .. } => Some(text.as_str()),
            ViewItem::Row { .. } => None,
        })
}

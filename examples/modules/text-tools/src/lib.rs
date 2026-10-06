//! A separately built module used to validate the complete module lifecycle.
use copypaste_module_sdk::{Module, ModuleEnvironment, ModuleInvocation, ModuleOutput};

pub struct TextTools;
impl Module for TextTools {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Ok(Self)
    }
    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        if invocation.command != "transform" {
            return Err("Unknown command.".into());
        }
        let text = invocation
            .arguments
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("Text is required.")?;
        let uppercase = invocation
            .preferences
            .get("uppercase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(ModuleOutput::Text {
            text: if uppercase {
                text.to_uppercase()
            } else {
                text.trim().to_owned()
            },
        })
    }
}
copypaste_module_sdk::export_module!(TextTools);

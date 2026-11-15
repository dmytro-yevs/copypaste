//! A separately built module used to validate the complete module lifecycle.
//! Hosts that grant it endpoints get every endpoint message echoed back.
use copypaste_module_sdk::{
    native::HostClient, Module, ModuleDelivery, ModuleEnvironment, ModuleHostRequest,
    ModuleInvocation, ModuleOutput,
};

pub struct TextTools {
    host: Option<HostClient>,
}
impl Module for TextTools {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Ok(Self { host: None })
    }
    fn create_with_host(_: ModuleEnvironment, host: HostClient) -> Result<Self, String> {
        Ok(Self { host: Some(host) })
    }
    fn deliver(&mut self, delivery: ModuleDelivery) -> Result<(), String> {
        let ModuleDelivery::EndpointFrame { connection, data } = delivery else {
            return Ok(());
        };
        self.host
            .as_ref()
            .ok_or("The host is missing.")?
            .call::<()>(&ModuleHostRequest::EndpointSend { connection, data })
            .map_err(|error| error.to_string())
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
copypaste_module_sdk::export_host_module!(TextTools);

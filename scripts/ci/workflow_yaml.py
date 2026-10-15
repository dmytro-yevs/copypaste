"""Read workflow YAML without YAML 1.1's implicit conversion of `on` to True."""

from pathlib import Path
import re

import yaml


class WorkflowLoader(yaml.SafeLoader):
    pass


WorkflowLoader.yaml_implicit_resolvers = {
    key: [(tag, pattern) for tag, pattern in resolvers
          if tag != "tag:yaml.org,2002:bool"]
    for key, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
}
WorkflowLoader.add_implicit_resolver("tag:yaml.org,2002:bool", re.compile(r"^(?:true|false)$", re.IGNORECASE), list("tTfF"))


def mapping(loader, node, deep=False):
    result = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in result:
            raise ValueError(f"duplicate YAML key: {key}")
        result[key] = loader.construct_object(value_node, deep=deep)
    return result


WorkflowLoader.add_constructor("tag:yaml.org,2002:map", mapping)


def read_workflow(path: Path) -> dict:
    return parse_workflow(path.read_text(encoding="utf-8"))


def parse_workflow(text: str) -> dict:
    document = yaml.load(text, Loader=WorkflowLoader)
    if not isinstance(document, dict):
        raise ValueError("expected a workflow mapping")
    return document

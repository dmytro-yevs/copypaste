// src/scripts/elementsContainedIn.ts
function elementsContainedIn(scope, elements) {
  function isInDocument(element) {
    let currentElement = element;
    while (currentElement && currentElement.parentNode) {
      if (currentElement.parentNode === scope || currentElement.parentNode.host === scope) {
        return true;
      } else if (currentElement.parentNode instanceof DocumentFragment) {
        currentElement = currentElement.parentNode.host;
      } else {
        currentElement = currentElement.parentNode;
      }
    }
    return false;
  }
  return elements.map((element) => isInDocument(element));
}
export {
  elementsContainedIn as default
};
